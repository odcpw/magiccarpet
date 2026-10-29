//! MC2 Summon Army holder, ring fan-out and family-owned creature handoff.
//!
//! Source model selectors stay at this adapter boundary. The ring remains the
//! source event until its life reaches zero; only then does the selected
//! creature owner allocate and bind a real `SummonedSpell` root.

use super::{
    EntitySnapshot, Level2SharedSpellResetSession, ParticleParameter, RuntimeSnapshot,
    SessionTerrain,
};
use crate::campaign::source_arena::{
    CreatureRootOrigin, CreatureRuntimeRole, SourceArena, SourceEntityHandle,
};
use crate::level_definition::{
    SummonArmyEnvironment, SummonArmyMember, SummonArmyProperties, SummonArmyTier,
};
use crate::native_world::MapType;
use crate::{FlightWorld, Position, move_entity};

const HOLDER_CLASS: u8 = 15;
const HOLDER_MODEL: u8 = 19;
const HOLDER_ACTION: u8 = 57;
const FLYER_CLASS: u8 = 9;
const FLYER_MODEL: u8 = 24;
const FLYER_ACTION: u8 = 25;
const FLYER_SPEED: i16 = 384;
const FLYER_MANA: i32 = 50;
const FLYER_MAX_LIFE: i32 = (0x2000 / 384) & 0xFC;
const FLYER_PARTICLE: u16 = 281;
const FLYER_IMPACT_CLASS: u8 = 10;
const FLYER_IMPACT_SUBTYPE: u8 = 72;
const RING_CLASS: u8 = 10;
const RING_MODEL: u8 = 72;
const RING_ACTION: u8 = 79;
const RING_LIFE: i32 = 16;
const RING_PARTICLE: u16 = 220;
const RING_SHIFT_ROTATION: i16 = 128;
const RING_FOV: i16 = 256;

pub(crate) const SUMMONED_STAGE_MODE: u8 = 13;
pub(crate) const SUMMONED_REMAINING_LIFETIME: u16 = 250;

/// Inputs from the ordinary spell dispatch and the live level session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CastInput {
    pub map_type: MapType,
    pub aim_yaw: i16,
    pub aim_pitch: i16,
    pub auxiliary_byte: u8,
    pub castle_mana: Option<i32>,
}

/// Source emissions and the caster side effect consumed by the shared tail.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct HolderOutcome {
    pub event_slot: Option<u16>,
    pub clear_player_aux: Option<u16>,
    pub sound: Option<(u16, u8)>,
    pub emission_failed: bool,
    pub expired: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RingGroupOutcome {
    pub requested: u8,
    pub rings: Vec<SourceEntityHandle>,
    pub allocation_failed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FlyerEmission {
    event_slot: u16,
    parent_slot: u16,
}

impl RingGroupOutcome {
    #[must_use]
    pub fn complete(&self) -> bool {
        !self.allocation_failed && self.rings.len() == usize::from(self.requested)
    }
}

/// Concrete family allocation request made by the ordinary ring callback.
/// The family owner must apply these source post-constructor fields and bind
/// `CreatureRootOrigin::SummonedSpell { holder_slot }` before returning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SummonedCreatureRequest {
    pub member: SummonArmyMember,
    pub position: Position,
    pub parent_slot: u16,
    pub owner_id: u16,
    pub holder_slot: u16,
    pub stage_mode: u8,
    pub remaining_lifetime: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RingStepOutcome {
    Waiting { remaining_life: i32 },
    Spawned { creature: SourceEntityHandle },
    NoCreatureSlot { member: SummonArmyMember },
    Hidden { life: i32 },
}

fn word(row: &EntitySnapshot, offset: usize) -> u16 {
    u16::from_le_bytes([row.record[offset], row.record[offset + 1]])
}

fn put_word(row: &mut EntitySnapshot, offset: usize, value: u16) {
    row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn set_ring_shift_rotation(row: &mut EntitySnapshot) {
    row.record[88..90].copy_from_slice(&RING_SHIFT_ROTATION.to_le_bytes());
    row.record[90..92].copy_from_slice(&RING_SHIFT_ROTATION.to_le_bytes());
    row.record[92..94].copy_from_slice(&RING_FOV.to_le_bytes());
}

fn ring_map_publication_order(requested: u8) -> impl Iterator<Item = u8> {
    (1..requested).chain(std::iter::once(0))
}

fn ring_position(
    flyer: &EntitySnapshot,
    index: u8,
    requested: u8,
    terrain: &SessionTerrain,
) -> Position {
    let mut position = flyer.position;
    move_entity(
        &mut position,
        i16::from(index) * (2048 / i16::from(requested)),
        0,
        512,
    );
    position.z = terrain.ground_height(position);
    position
}

fn set_owner(row: &mut EntitySnapshot, owner_id: u16) {
    row.source_id = owner_id;
    put_word(row, 30, owner_id);
}

fn environment(map_type: MapType) -> SummonArmyEnvironment {
    match map_type {
        MapType::Day => SummonArmyEnvironment::Day,
        MapType::Night => SummonArmyEnvironment::Night,
        MapType::Cave => SummonArmyEnvironment::Cave,
    }
}

fn family_selector(
    level_gate: i8,
    map_type: MapType,
    properties: SummonArmyProperties,
) -> Result<u8, String> {
    let tier = SummonArmyTier::from_level_gate(level_gate)
        .ok_or("Summon Army has an unsupported source tier")?;
    let selection = properties
        .select(tier, environment(map_type))
        .ok_or("Summon Army tier has no creature group")?;
    Ok(source_model(selection.member))
}

/// Source `life_0x1A` values copied by `sub_6C170` into byte46.
const fn source_model(member: SummonArmyMember) -> u8 {
    match member {
        SummonArmyMember::Bee => 2,
        SummonArmyMember::Firefly => 19,
        SummonArmyMember::Cymmerian => 25,
        SummonArmyMember::Wyvern => 16,
    }
}

fn source_member(selector: u8) -> Option<SummonArmyMember> {
    match selector {
        2 => Some(SummonArmyMember::Bee),
        19 => Some(SummonArmyMember::Firefly),
        25 => Some(SummonArmyMember::Cymmerian),
        16 => Some(SummonArmyMember::Wyvern),
        _ => None,
    }
}

/// `sub_67800` selects the class-10/model-72 ring count from byte46, not from
/// the flyer subspell-index word.
const fn source_group_size(selector: u8) -> Option<u8> {
    match selector {
        2 | 19 => Some(8),
        25 => Some(4),
        16 => Some(2),
        _ => None,
    }
}

/// Source `sub_68E50(parent, flyer, holder)` lateral spawn offset for the
/// ordinary class-15/model-19 Summon Army holder.
fn flyer_spawn_position(
    parent: &EntitySnapshot,
    initial_position: Position,
    terrain: &SessionTerrain,
) -> Position {
    let side_flags = (parent.control_flags >> 8) as u8;
    let offset_yaw = if side_flags & 1 != 0 {
        parent.yaw.wrapping_sub(512) & 0x7ff
    } else if side_flags & 2 != 0 {
        parent.yaw.wrapping_add(512) & 0x7ff
    } else {
        return initial_position;
    };

    let mut position = initial_position;
    move_entity(&mut position, offset_yaw, 0, 256);
    if terrain.ground_height(position) > position.z {
        initial_position
    } else {
        position
    }
}

fn emit_flyer(
    shared: &Level2SharedSpellResetSession,
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    holder_slot: u16,
    initial_parent_slot: u16,
    input: CastInput,
    properties: SummonArmyProperties,
) -> Result<Option<FlyerEmission>, String> {
    // The raw source parent field is only a slot. Preserve its generation
    // before NewEvent can allocate or otherwise change the arena.
    let original_parent = arena
        .handle(initial_parent_slot)
        .ok_or("Summon Army caster has no live source handle")?;
    let initial_position = arena
        .entities
        .get(usize::from(initial_parent_slot))
        .filter(|row| row.active)
        .map(|parent| parent.position)
        .ok_or("Summon Army caster disappeared before flyer construction")?;
    let particle = particles
        .get(usize::from(FLYER_PARTICLE))
        .copied()
        .ok_or("Summon Army requires retained particle281")?;
    if particle.animation_limit().is_none() {
        return Err(format!(
            "Summon Army particle{FLYER_PARTICLE} has an unsupported render mode {}",
            particle.render_mode
        ));
    }
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let mut flyer =
        super::cave_features::new_event(slot, arena.economy.new_event_rng(slot), initial_position);
    flyer.class = FLYER_CLASS;
    flyer.model = FLYER_MODEL;
    flyer.record[67] = FLYER_CLASS;
    flyer.record[68] = FLYER_MODEL;
    flyer.set_source_action(FLYER_ACTION);
    flyer.set_actual_speed(FLYER_SPEED);
    flyer.record[136..138].copy_from_slice(&FLYER_SPEED.to_le_bytes());
    flyer.set_mana(FLYER_MANA);
    flyer.set_max_life(FLYER_MAX_LIFE);
    flyer.set_control_flags(flyer.control_flags & !8);
    flyer.parameter_relation = Some(60);
    flyer.record[164..168].copy_from_slice(&61_u32.to_le_bytes());

    arena.economy.source_rng.insert(slot, flyer.entity_rng);
    arena.entities[usize::from(slot)] = flyer;
    crate::world::effects::sync_position(arena, terrain, slot, initial_position, true, false)?;
    let mut flyer = arena.entities[usize::from(slot)].clone();
    flyer.set_life(FLYER_MAX_LIFE);
    super::cave_features::bind_particle(&mut flyer, FLYER_PARTICLE, particle)?;
    arena.entities[usize::from(slot)] = flyer;

    // sub_6C170 reads the holder and caster again after sub_4E050 allocates
    // and publishes the flyer. Keep that ordering so a replacement slot or
    // callback-side update cannot leak the pre-allocation row into the tail.
    let holder = shared
        .pickup(holder_slot)
        .filter(|row| {
            row.active
                && (row.class, row.model, row.action) == (HOLDER_CLASS, HOLDER_MODEL, HOLDER_ACTION)
        })
        .cloned()
        .ok_or("Summon Army holder disappeared during flyer construction")?;
    let parent_slot = holder.source_spell_parent_entity_index();
    let parent = arena
        .entities
        .get(usize::from(parent_slot))
        .filter(|row| row.active)
        .cloned()
        .ok_or("Summon Army caster disappeared during flyer construction")?;
    let caster_handle = if parent_slot == initial_parent_slot {
        original_parent
    } else {
        arena
            .handle(parent_slot)
            .ok_or("Summon Army replacement caster has no live source handle")?
    };
    let selector = family_selector(holder.level_gate, input.map_type, properties)?;

    let mut flyer = arena.entities[usize::from(slot)].clone();
    flyer.set_actual_speed(flyer.actual_speed.wrapping_add(parent.actual_speed));
    let position = flyer_spawn_position(&parent, initial_position, terrain);
    flyer.position = position;
    arena.entities[usize::from(slot)] = flyer;
    if position != initial_position {
        crate::world::effects::sync_position(arena, terrain, slot, position, true, true)?;
    }

    let mut flyer = arena.entities[usize::from(slot)].clone();
    flyer.record[71] = FLYER_IMPACT_CLASS;
    flyer.record[72] = FLYER_IMPACT_SUBTYPE;
    set_owner(&mut flyer, parent.source_id);
    flyer.set_source_byte_46(selector);
    flyer.set_source_dword_10(i32::from(input.auxiliary_byte));
    put_word(&mut flyer, 42, holder_slot);
    put_word(&mut flyer, 44, parent_slot);

    let yaw = input.aim_yaw.wrapping_add(parent.yaw);
    let pitch = input.aim_pitch.wrapping_add(parent.pitch);
    flyer.set_yaw(yaw);
    flyer.set_pitch(pitch);
    let mut axis = parent.position;
    move_entity(&mut axis, yaw, pitch, 0x4000);
    put_word(&mut flyer, 158, axis.x);
    put_word(&mut flyer, 160, axis.y);
    put_word(&mut flyer, 162, axis.z as u16);

    arena.entities[usize::from(slot)] = flyer;
    let flyer_handle = arena
        .handle(slot)
        .ok_or("Summon Army flyer lost its source handle")?;
    if !arena.bind_carried_creature_parent(flyer_handle, caster_handle) {
        return Err("Summon Army flyer lost its caster generation".to_owned());
    }
    Ok(Some(FlyerEmission {
        event_slot: slot,
        parent_slot,
    }))
}

/// Source `sub_6C170` for spell 19: AddEvent class 9/subtype 0x18 produces the
/// 9/24/25 flyer, whose byte43/byte44 impact payload requests class 10/subtype
/// 72. The root scheduler owns the pass tail and next-pass reclaim.
pub(crate) fn step_holder(
    shared: &mut Level2SharedSpellResetSession,
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    input: CastInput,
    properties: SummonArmyProperties,
) -> Result<HolderOutcome, String> {
    let mut holder = shared
        .pickup(slot)
        .cloned()
        .ok_or("Summon Army holder is absent")?;
    if (holder.class, holder.model, holder.action) != (HOLDER_CLASS, HOLDER_MODEL, HOLDER_ACTION) {
        return Err("Summon Army holder has a different source selector".to_owned());
    }
    arena.entities[usize::from(slot)] = holder.clone();
    let parent_slot = holder.source_spell_parent_entity_index();
    let mut outcome = HolderOutcome::default();

    if holder.active_value > 0 {
        if parent_slot != 0
            && let Some(parent) = arena.entities.get(usize::from(parent_slot)).cloned()
        {
            let admitted = parent.mana >= 0
                && parent.life >= 0
                && (holder.mana_regen == 0
                    || input
                        .castle_mana
                        .is_some_and(|mana| mana >= holder.mana_regen))
                && (holder.active_value != holder.activation_value as i16
                    || parent.mana >= holder.max_mana);
            if admitted {
                if holder.active_value == holder.activation_value as i16 {
                    let _ = family_selector(holder.level_gate, input.map_type, properties)?;
                    let emission = emit_flyer(
                        shared,
                        arena,
                        terrain,
                        particles,
                        slot,
                        parent_slot,
                        input,
                        properties,
                    )?;
                    outcome.event_slot = emission.map(|emission| emission.event_slot);
                    outcome.emission_failed = outcome.event_slot.is_none();
                    if let Some(emission) = emission {
                        outcome.clear_player_aux = Some(emission.parent_slot);
                        outcome.sound = Some((emission.event_slot, 9));
                    }
                }

                // sub_68DE0 runs on every admitted visit, including failed
                // NewEvent allocation, and does not directly subtract mana.
                if let Some(parent) = arena.entities.get_mut(usize::from(parent_slot)) {
                    let regen = if holder.active_value == holder.activation_value as i16 {
                        if parent.mana_regen >= 0 {
                            holder.max_mana.wrapping_neg()
                        } else {
                            parent.mana_regen.wrapping_sub(holder.max_mana)
                        }
                    } else if parent.mana_regen > 0 {
                        0
                    } else {
                        parent.mana_regen
                    };
                    parent.set_mana_regen(regen);
                    shared.sync_player_entity_from_authority(parent);
                    if parent_slot == shared.player_entity.index {
                        shared.sync_typed_spell_parent_caches_from_player_entity();
                    }
                }
            } else {
                holder.active_value = 1;
            }
        }

        holder.active_value = holder.active_value.wrapping_sub(1);
        if holder.active_value == 0 {
            let deferred = holder.source_word_2c();
            if deferred != 0 {
                let tier =
                    crate::spell_activation::level2_applied_tier(deferred.wrapping_sub(1) as i8)
                        .ok_or("Summon Army deferred tier is negative")?;
                let profile = crate::spell_activation::level2_set_spell_profile(HOLDER_MODEL, tier)
                    .ok_or("Summon Army deferred tier has no source profile")?;
                holder.apply_nyphur_spell_profile(profile, tier);
                holder.set_source_word_2c(0);
            }
            outcome.expired = true;
        }
    }

    let cooldown = word(&holder, 58) as i16;
    if cooldown != 0 {
        put_word(&mut holder, 58, cooldown.wrapping_sub(1) as u16);
    }
    holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
    if arena.entities[usize::from(slot)].class == HOLDER_CLASS
        && arena.entities[usize::from(slot)].model == HOLDER_MODEL
    {
        if let Some(current) = shared.pickup_mut(slot) {
            *current = holder.clone();
        }
        arena.entities[usize::from(slot)] = holder;
    }
    Ok(outcome)
}

/// Source `sub_51800` fan-out from the terminal 9/24/25 flyer after
/// `sub_65820` consumes its 10/72 impact payload. It creates
/// the 8/4/2 class10/model72/action79 rings around the real impact position;
/// each ring is consumed later by `step_ring`.
pub(crate) fn construct_ring_group(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    flyer_slot: u16,
) -> Result<RingGroupOutcome, String> {
    let flyer = arena
        .entities
        .get(usize::from(flyer_slot))
        .filter(|row| {
            row.active
                && (row.class, row.model, row.action) == (FLYER_CLASS, FLYER_MODEL, FLYER_ACTION)
        })
        .cloned()
        .ok_or_else(|| format!("Summon Army flyer callback lost slot{flyer_slot}"))?;
    let selector = flyer.record[74];
    if source_member(selector).is_none() {
        return Err(format!(
            "Summon Army flyer has unsupported family selector {selector}"
        ));
    }
    let requested = source_group_size(selector)
        .ok_or("Summon Army flyer has no source ring count for its family selector")?;
    let mut outcome = RingGroupOutcome {
        requested,
        rings: Vec::with_capacity(usize::from(requested)),
        allocation_failed: false,
    };
    if arena.economy.free_slot_count() < usize::from(requested) {
        outcome.allocation_failed = true;
        return Ok(outcome);
    }
    let holder_slot = word(&flyer, 42);
    let parent_slot = word(&flyer, 44);
    let parent_binding = arena
        .handle(flyer_slot)
        .and_then(|handle| arena.creature_parent_binding(handle))
        .filter(|parent| parent.source_slot == parent_slot);
    let owner_id = flyer.source_id;
    let particle = particles
        .get(usize::from(RING_PARTICLE))
        .copied()
        .ok_or("Summon Army requires retained particle220")?;

    if particle.animation_limit().is_none() {
        return Err(format!(
            "Summon Army particle{RING_PARTICLE} has an unsupported render mode {}",
            particle.render_mode
        ));
    }

    // sub_51800 allocates the first ring before its children. Each child
    // qmemcpy copies that first row, including its newborn RNG bytes.
    let first_slot = arena
        .allocate_source_slot_with_terrain(terrain)
        .ok_or("Summon Army ring allocation failed after source-capacity preflight")?;
    let mut first = super::cave_features::new_event(
        first_slot,
        arena.economy.new_event_rng(first_slot),
        flyer.position,
    );
    first.class = RING_CLASS;
    first.model = RING_MODEL;
    first.record[67] = RING_CLASS;
    first.record[68] = RING_MODEL;
    first.set_source_action(RING_ACTION);
    first.set_control_flags(first.control_flags & !8);
    first.set_max_life(RING_LIFE);
    first.set_life(RING_LIFE);
    first.set_phase(0);
    set_owner(&mut first, owner_id);
    first.set_source_byte_46(selector);
    put_word(&mut first, 42, holder_slot);
    put_word(&mut first, 44, parent_slot);
    arena
        .economy
        .source_rng
        .insert(first_slot, first.entity_rng);
    arena.entities[usize::from(first_slot)] = first;

    let mut slots = Vec::with_capacity(usize::from(requested));
    slots.push(first_slot);
    let mut previous = first_slot;

    // sub_51800 adds indices 1..N-1 to their map tiles before publishing the
    // first ring. This makes the first ring's tile-head position source-exact.
    for index in ring_map_publication_order(requested) {
        if index == 0 {
            let mut first = arena.entities[usize::from(first_slot)].clone();
            first.set_position(ring_position(&flyer, 0, requested, terrain));
            arena.entities[usize::from(first_slot)] = first;
            let position = arena.entities[usize::from(first_slot)].position;
            crate::world::effects::sync_position(
                arena, terrain, first_slot, position, true, false,
            )?;

            let mut first = arena.entities[usize::from(first_slot)].clone();
            set_ring_shift_rotation(&mut first);
            super::cave_features::bind_particle(&mut first, RING_PARTICLE, particle)?;
            arena.entities[usize::from(first_slot)] = first;
            continue;
        }

        let slot = arena
            .allocate_source_slot_with_terrain(terrain)
            .ok_or("Summon Army child allocation failed after source-capacity preflight")?;
        let mut ring = arena.entities[usize::from(first_slot)].clone();
        ring.index = slot;
        ring.record[0..2].copy_from_slice(&slot.to_le_bytes());
        ring.record[2] = 1;
        ring.set_phase(index);
        put_word(&mut ring, 54, previous);
        put_word(&mut ring, 56, 0);
        ring.next_relation = None;
        ring.set_position(ring_position(&flyer, index, requested, terrain));

        let predecessor = arena
            .entities
            .get_mut(usize::from(previous))
            .ok_or_else(|| {
                format!("Summon Army ring predecessor {previous} is outside the arena")
            })?;
        put_word(predecessor, 56, slot);
        predecessor.next_relation = Some(slot);

        arena.economy.source_rng.insert(slot, ring.entity_rng);
        arena.entities[usize::from(slot)] = ring;
        let position = arena.entities[usize::from(slot)].position;
        crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;

        let mut ring = arena.entities[usize::from(slot)].clone();
        set_ring_shift_rotation(&mut ring);
        super::cave_features::bind_particle(&mut ring, RING_PARTICLE, particle)?;
        arena.entities[usize::from(slot)] = ring;

        slots.push(slot);
        previous = slot;
    }

    for slot in slots {
        let handle = arena
            .handle(slot)
            .ok_or_else(|| format!("Summon Army ring lost source slot{slot}"))?;
        if let Some(parent) = parent_binding
            && !arena.bind_carried_creature_parent(handle, parent)
        {
            return Err(format!(
                "Summon Army ring {slot} lost its caster generation"
            ));
        }
        outcome.rings.push(handle);
    }
    Ok(outcome)
}

fn ring_chain_slots(arena: &SourceArena, first_ring_slot: u16) -> Result<Vec<u16>, String> {
    // The group was sized from the selector at construction time. The flyer
    // can subsequently carry a different selector, which sub_67800 copies
    // over the existing chain without changing its length.
    const MAX_SOURCE_RINGS: usize = 8;
    let mut slots = Vec::with_capacity(MAX_SOURCE_RINGS);
    let mut cursor = first_ring_slot;
    let mut cycle = None;
    while cursor != 0 && slots.len() < MAX_SOURCE_RINGS {
        if slots.contains(&cursor) {
            cycle = Some(cursor);
            break;
        }
        let ring = arena
            .entities
            .get(usize::from(cursor))
            .filter(|row| {
                row.active
                    && (row.class, row.model, row.action) == (RING_CLASS, RING_MODEL, RING_ACTION)
            })
            .ok_or_else(|| format!("Summon Army ring chain lost slot{cursor}"))?;
        slots.push(cursor);
        cursor = word(ring, 56);
    }
    if let Some(slot) = cycle {
        return Err(format!("Summon Army ring chain cycles at slot{slot}"));
    }
    if cursor != 0 {
        return Err("Summon Army ring chain exceeds its source fan-out".to_owned());
    }
    if !matches!(slots.len(), 2 | 4 | 8) {
        return Err(format!(
            "Summon Army ring chain has invalid source fan-out of {} entries",
            slots.len()
        ));
    }
    Ok(slots)
}

/// `sub_67800` rereads its current flyer after `sub_65820` constructs the
/// group, then copies byte46, parentId and id over every event reached through
/// word34. Build the whole chain first so a malformed link cannot leave a
/// partially-propagated group.
fn propagate_current_flyer_to_ring_chain(
    arena: &mut SourceArena,
    flyer_slot: u16,
    first_ring_slot: u16,
) -> Result<usize, String> {
    let flyer = arena
        .entities
        .get(usize::from(flyer_slot))
        .filter(|row| {
            row.active
                && (row.class, row.model, row.action) == (FLYER_CLASS, FLYER_MODEL, FLYER_ACTION)
        })
        .cloned()
        .ok_or_else(|| format!("Summon Army post-guided flyer lost slot{flyer_slot}"))?;
    let selector = flyer.record[74];
    source_member(selector)
        .ok_or_else(|| format!("Summon Army flyer has unsupported family selector {selector}"))?;
    let slots = ring_chain_slots(arena, first_ring_slot)?;
    let parent_id = word(&flyer, 44);
    let owner_id = flyer.source_id;

    for slot in slots.iter().copied() {
        let ring = &mut arena.entities[usize::from(slot)];
        ring.set_source_byte_46(selector);
        put_word(ring, 44, parent_id);
        set_owner(ring, owner_id);
    }
    Ok(slots.len())
}

/// Guided-projectile impact hook for the exact 10/72 flyer payload. The
/// shared guided consumer keeps its ordinary current-row post-constructor
/// propagation on the first ring returned here.
pub(crate) fn construct_ring_impact(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    flyer_slot: u16,
) -> Result<Option<u16>, String> {
    let group = construct_ring_group(arena, terrain, particles, flyer_slot)?;
    if group.allocation_failed {
        if !group.rings.is_empty() {
            return Err(
                "Summon Army ring group partially allocated below source capacity".to_owned(),
            );
        }
        return Ok(None);
    }
    if !group.complete() {
        return Err("Summon Army ring group completed an incomplete source batch".to_owned());
    }
    Ok(group.rings.first().map(|ring| ring.source_slot))
}

/// Source 9/24/25 callback around the shared sub_65820 projectile handler.
/// It has a fixed cast axis and no acquired target; the shared handler returns
/// the current 10/72 constructor row after the ring hook and copies that row's
/// ownership/aim/subspell payload onto the first ring.
pub(crate) fn step_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    targets: Option<&super::guided_projectile::Targets<'_>>,
    context: super::source_target::Context<'_>,
) -> Result<super::guided_projectile::StepOutcome, String> {
    let outcome = super::guided_projectile::step(
        arena,
        terrain,
        particles,
        slot,
        super::guided_projectile::Guidance {
            yaw_turn: 0,
            pitch_turn: 0,
        },
        targets,
        context,
    )?;
    if let Some(first_ring_slot) = outcome.impact_slot {
        propagate_current_flyer_to_ring_chain(arena, slot, first_ring_slot)?;
    }
    Ok(outcome)
}

/// `sub_3A5B0` consumes each ordinary 10/72/79 ring. The creature constructor
/// is called only on the visit that decrements life to exactly zero. It must
/// perform the family-owned initialization, source post-fields and provenance
/// binding before returning its generation-bound handle.
pub(crate) fn step_ring<F>(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    mut construct_member: F,
) -> Result<RingStepOutcome, String>
where
    F: FnMut(
        &mut SourceArena,
        &mut RuntimeSnapshot,
        &mut SessionTerrain,
        &[ParticleParameter],
        SummonedCreatureRequest,
    ) -> Result<Option<SourceEntityHandle>, String>,
{
    let mut ring = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| {
            row.active
                && (row.class, row.model, row.action) == (RING_CLASS, RING_MODEL, RING_ACTION)
        })
        .cloned()
        .ok_or_else(|| format!("Summon Army ring callback lost slot{slot}"))?;
    ring.set_life(ring.life.wrapping_sub(1));
    if ring.life < 0 {
        ring.disable_drawing();
        arena.entities[usize::from(slot)] = ring.clone();
        return Ok(RingStepOutcome::Hidden { life: ring.life });
    }

    // Publish the zero boundary before NewEvent can reuse any fixed address.
    arena.entities[usize::from(slot)] = ring.clone();
    if ring.life != 0 {
        return Ok(RingStepOutcome::Waiting {
            remaining_life: ring.life,
        });
    }

    let selector = ring.record[74];
    let member = source_member(selector)
        .ok_or_else(|| format!("Summon Army ring has unsupported family selector {selector}"))?;
    let request = SummonedCreatureRequest {
        member,
        position: ring.position,
        parent_slot: word(&ring, 44),
        owner_id: ring.source_id,
        holder_slot: word(&ring, 42),
        stage_mode: SUMMONED_STAGE_MODE,
        remaining_lifetime: SUMMONED_REMAINING_LIFETIME,
    };
    let parent_binding = arena
        .handle(slot)
        .and_then(|handle| arena.creature_parent_binding(handle))
        .filter(|parent| parent.source_slot == request.parent_slot);
    let Some(creature) = construct_member(arena, runtime, terrain, particles, request)? else {
        return Ok(RingStepOutcome::NoCreatureSlot { member });
    };
    if arena.resolve(creature).is_none() {
        return Err("Summon Army family constructor returned a stale source handle".to_owned());
    }
    if let Some(parent) = parent_binding {
        if !arena.bind_carried_creature_parent(creature, parent) {
            return Err("Summon Army child lost its caster generation".to_owned());
        }
        if request.member == SummonArmyMember::Firefly {
            super::level2_class5::firefly::inherit_summoned_parent_generation(
                arena, creature, parent,
            )?;
        }
    }
    let expected_kind = match request.member {
        SummonArmyMember::Bee => crate::level_definition::CreatureKind::Bee,
        SummonArmyMember::Firefly => crate::level_definition::CreatureKind::Firefly,
        SummonArmyMember::Cymmerian => crate::level_definition::CreatureKind::Cymmerian,
        SummonArmyMember::Wyvern => crate::level_definition::CreatureKind::Wyvern,
    };
    let identity = arena
        .creature_runtime_identity(
            creature.source_slot,
            &std::collections::BTreeMap::new(),
            &std::collections::BTreeMap::new(),
        )
        .ok_or("Summon Army family constructor did not bind a creature identity")?;
    if identity.handle != creature
        || identity.kind != expected_kind
        || identity.root_slot != creature.source_slot
        || identity.role != CreatureRuntimeRole::Root
        || identity.origin
            != (CreatureRootOrigin::SummonedSpell {
                holder_slot: request.holder_slot,
            })
    {
        return Err(format!(
            "Summon Army {:?} constructor returned a creature without its holder provenance",
            request.member
        ));
    }
    let expected_action = selector.wrapping_mul(8).wrapping_add(7);
    let post_fields_match = match request.member {
        SummonArmyMember::Bee => arena
            .economy
            .bee_creatures()
            .iter()
            .find(|child| child.active && child.source_entity_index == creature.source_slot)
            .is_some_and(|child| {
                child.source_class == 5
                    && child.model == selector
                    && child.action == expected_action
                    && child.stage_var2 == request.stage_mode
                    && child.parent_entity_index == request.parent_slot
                    && child.entity_id == request.owner_id
                    && child.stage_remaining.cast_unsigned() == request.remaining_lifetime
                    && child.mana == 0
                    && child.origin
                        == (super::creatures::bee::BeeCreatureOrigin::SummonedSpell {
                            holder_slot: request.holder_slot,
                        })
            }),
        SummonArmyMember::Firefly => {
            let source =
                arena.economy.authored_class5().iter().find(|source| {
                    source.active && source.source_entity_index == creature.source_slot
                });
            let sidecar = arena
                .economy
                .authored_fireflies()
                .iter()
                .find(|state| state.source_entity_index == creature.source_slot);
            source.zip(sidecar).is_some_and(|(source, sidecar)| {
                source.model == selector
                    && source.action == expected_action
                    && source.mana == 0
                    && source.max_mana == 0
                    && source.word_0x96_150 == 0
                    // sub_3A5B0 writes id_0x1A from the ring; NewEvent's
                    // separate playerEntityIndex_0x94 remains zero.
                    && sidecar.entity_id == request.owner_id
                    && arena.economy.source_owner(creature.source_slot) == 0
                    && sidecar.authored_index.is_none()
                    && sidecar.stage_var2 == request.stage_mode
                    && sidecar.parent_entity_index == request.parent_slot
                    && sidecar.word_0x2e == request.remaining_lifetime
            })
        }
        SummonArmyMember::Cymmerian | SummonArmyMember::Wyvern => arena
            .entities
            .get(usize::from(creature.source_slot))
            .filter(|child| child.active)
            .is_some_and(|child| {
                child.class == 5
                    && child.model == selector
                    && child.action == expected_action
                    && child.record[77] == request.stage_mode
                    && word(child, 44) == request.parent_slot
                    && word(child, 50) == request.remaining_lifetime
                    && word(child, 154) == 0
                    && child.mana == 0
                    && child.max_mana == 0
                    && child.source_id == request.owner_id
            }),
    };
    if !post_fields_match {
        return Err(format!(
            "Summon Army {:?} constructor returned a carrier without source post-construction state",
            request.member
        ));
    }
    Ok(RingStepOutcome::Spawned { creature })
}

/// Dispatch the terminal source-ring visit to the selected family owner.
/// Each constructor allocates and initializes its real typed/raw carrier; the
/// shared ring consumer validates its holder provenance and source fields.
pub(crate) fn step_ring_with_family(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
) -> Result<RingStepOutcome, String> {
    step_ring(
        arena,
        runtime,
        terrain,
        particles,
        slot,
        |arena, runtime, terrain, particles, request| match request.member {
            SummonArmyMember::Bee => super::creatures::bee::mc2::allocate_summoned(
                arena,
                runtime,
                terrain,
                particles,
                request.position,
                request.parent_slot,
                request.owner_id,
                request.holder_slot,
            ),
            SummonArmyMember::Firefly => super::level2_class5::firefly::allocate_summoned(
                arena,
                runtime,
                terrain,
                particles,
                request.position,
                request.parent_slot,
                request.owner_id,
                request.holder_slot,
            ),
            SummonArmyMember::Cymmerian => super::creatures::creature_25::mc2::allocate_summoned(
                arena,
                runtime,
                terrain,
                particles,
                request.position,
                request.parent_slot,
                request.owner_id,
                request.holder_slot,
            ),
            SummonArmyMember::Wyvern => super::creatures::creature_16::mc2::allocate_summoned(
                arena,
                runtime,
                terrain,
                particles,
                request.position,
                request.parent_slot,
                request.owner_id,
                request.holder_slot,
            ),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring_fixture(
        free_slots: usize,
        selector: u8,
    ) -> (SourceArena, SessionTerrain, Vec<ParticleParameter>, u16) {
        let plane = || vec![0; 65_536].into_boxed_slice();
        let terrain = SessionTerrain::from_planes(
            MapType::Day,
            crate::native_world::TerrainPlanes {
                terrain_types: plane(),
                ground_heights: plane(),
                shading: plane(),
                angles: plane(),
                secondary_heights: plane(),
                entity_indices: vec![0; 65_536].into_boxed_slice(),
            },
            0,
        );
        let position = Position {
            x: 32_768,
            y: 32_768,
            z: 1_024,
        };
        let mut entities = Vec::with_capacity(free_slots + 2);
        entities.push(super::super::cave_features::new_event(0, 7, position));
        let mut flyer = super::super::cave_features::new_event(1, 13, position);
        flyer.class = FLYER_CLASS;
        flyer.model = FLYER_MODEL;
        flyer.record[67] = FLYER_CLASS;
        flyer.record[68] = FLYER_MODEL;
        flyer.set_source_action(FLYER_ACTION);
        flyer.record[74] = selector;
        flyer.set_source_byte_46(selector);
        set_owner(&mut flyer, 42);
        put_word(&mut flyer, 42, 5);
        put_word(&mut flyer, 44, 6);
        entities.push(flyer);
        for slot in 2..(free_slots + 2) {
            let mut row = super::super::cave_features::new_event(slot as u16, 0, position);
            row.reclaim_source_row();
            entities.push(row);
        }
        let economy = crate::native_world::Level2EconomyState::from_parts(
            terrain.clone(),
            &entities,
            0,
            0,
            0,
            0,
            1,
        )
        .expect("small source arena fixture is valid");
        let arena = SourceArena::new(entities.into_boxed_slice(), economy);
        let particle = ParticleParameter {
            base_tmap: 0,
            word_2: 0,
            word_4: 0,
            speed: 256,
            rotation_speed: 512,
            byte_10: 0,
            byte_11: 0,
            render_mode: 20,
            byte_13: 0,
        };
        let mut particles = vec![particle; 291];
        particles[usize::from(RING_PARTICLE)] = particle;
        (arena, terrain, particles, 1)
    }

    fn campaign_runtime_fixture() -> RuntimeSnapshot {
        let package_root = std::env::var_os("MC_ORDINARY_CAMPAIGN_PACKAGES").map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj")
            },
            std::path::PathBuf::from,
        );
        let package = crate::campaign::spec::DecodedCampaignPackage::decode(
            package_root.join("level18"),
            crate::campaign::spec::CampaignSelection::ordinary(18)
                .expect("ordinary Mission 18 selection"),
        )
        .expect("decode owner-data Mission 18 campaign package");
        let runtime = crate::native_world::parse_runtime(
            package
                .member("runtime.bin")
                .expect("Mission 18 retains its runtime member"),
        )
        .expect("Mission 18 runtime is source-valid");
        assert!(
            runtime.constructor_counters().is_some(),
            "Mission 18 runtime retains the source constructor counter table"
        );
        runtime
    }

    #[test]
    fn flyer_family_selects_source_ring_count() {
        assert_eq!(source_group_size(2), Some(8));
        assert_eq!(source_group_size(19), Some(8));
        assert_eq!(source_group_size(25), Some(4));
        assert_eq!(source_group_size(16), Some(2));
    }

    #[test]
    fn unsupported_source_family_has_no_ring_count() {
        assert_eq!(source_group_size(u8::MAX), None);
    }

    #[test]
    fn ring_group_exhaustion_preflights_without_allocating_a_partial_group() {
        let (mut arena, mut terrain, particles, flyer) = ring_fixture(7, 2);
        let free_before = arena.economy.free_slot_count();

        let group = construct_ring_group(&mut arena, &mut terrain, &particles, flyer).unwrap();

        assert!(group.allocation_failed);
        assert!(group.rings.is_empty());
        assert_eq!(arena.economy.free_slot_count(), free_before);
        assert_eq!(
            arena
                .entities
                .iter()
                .filter(|row| (row.class, row.model, row.action)
                    == (RING_CLASS, RING_MODEL, RING_ACTION))
                .count(),
            0
        );
    }

    #[test]
    fn ring_children_clone_first_ring_rng_and_publish_before_ring_zero() {
        let (mut arena, mut terrain, particles, flyer) = ring_fixture(8, 2);
        assert_eq!(
            ring_map_publication_order(8).collect::<Vec<_>>(),
            [1, 2, 3, 4, 5, 6, 7, 0]
        );

        let group = construct_ring_group(&mut arena, &mut terrain, &particles, flyer).unwrap();

        assert!(group.complete());
        assert_eq!(group.rings.len(), 8);
        assert_eq!(arena.economy.free_slot_count(), 0);
        let slots = group
            .rings
            .iter()
            .map(|ring| ring.source_slot)
            .collect::<Vec<_>>();
        let first_rng = arena.entities[usize::from(slots[0])].entity_rng;
        for (index, slot) in slots.iter().copied().enumerate() {
            let ring = &arena.entities[usize::from(slot)];
            let previous = index
                .checked_sub(1)
                .map(|previous| slots[previous])
                .unwrap_or(0);
            let next = slots.get(index + 1).copied().unwrap_or(0);
            assert_eq!(
                (ring.class, ring.model, ring.action),
                (RING_CLASS, RING_MODEL, RING_ACTION)
            );
            assert_eq!((ring.life, ring.max_life), (RING_LIFE, RING_LIFE));
            assert_eq!(ring.phase, index as u8);
            assert_eq!(ring.record[66], index as u8);
            assert_eq!(ring.entity_rng, first_rng);
            assert_eq!(&ring.record[24..26], &first_rng.to_le_bytes());
            assert_eq!(word(ring, 54), previous);
            assert_eq!(word(ring, 56), next);
            assert_eq!(ring.next_relation, (next != 0).then_some(next));
            assert_eq!(ring.particle_parameter_index, Some(RING_PARTICLE));
            assert_eq!(i16::from_le_bytes([ring.record[88], ring.record[89]]), 128);
            assert_eq!(i16::from_le_bytes([ring.record[90], ring.record[91]]), 128);
            assert_eq!(i16::from_le_bytes([ring.record[92], ring.record[93]]), 256);
        }
        let first_tile = arena.economy.map_chains.tile_of(slots[0]);
        assert_eq!(arena.economy.map_chains.tile_head(first_tile), slots[0]);
    }

    #[test]
    fn post_guided_flyer_payload_reaches_every_word34_ring() {
        let (mut arena, mut terrain, particles, flyer_slot) = ring_fixture(8, 19);
        let group = construct_ring_group(&mut arena, &mut terrain, &particles, flyer_slot).unwrap();
        assert!(group.complete());

        let mut current_flyer = arena.entities[usize::from(flyer_slot)].clone();
        current_flyer.set_source_byte_46(2);
        put_word(&mut current_flyer, 44, 0x4567);
        set_owner(&mut current_flyer, 0x2345);
        arena.entities[usize::from(flyer_slot)] = current_flyer;

        assert_eq!(
            propagate_current_flyer_to_ring_chain(
                &mut arena,
                flyer_slot,
                group.rings[0].source_slot
            )
            .unwrap(),
            8
        );
        for handle in group.rings {
            let ring = &arena.entities[usize::from(handle.source_slot)];
            assert_eq!(ring.record[74], 2);
            assert_eq!(word(ring, 44), 0x4567);
            assert_eq!(ring.source_id, 0x2345);
            assert_eq!(word(ring, 30), 0x2345);
        }
    }

    #[test]
    fn post_guided_selector_change_preserves_the_constructed_chain_size() {
        let (mut arena, mut terrain, particles, flyer_slot) = ring_fixture(8, 19);
        let group = construct_ring_group(&mut arena, &mut terrain, &particles, flyer_slot).unwrap();
        assert!(group.complete());
        arena.entities[usize::from(flyer_slot)].set_source_byte_46(25);

        assert_eq!(
            propagate_current_flyer_to_ring_chain(
                &mut arena,
                flyer_slot,
                group.rings[0].source_slot,
            )
            .unwrap(),
            8
        );
        for handle in group.rings {
            assert_eq!(
                arena.entities[usize::from(handle.source_slot)].record[74],
                25
            );
        }
    }

    #[test]
    fn post_guided_payload_rejects_a_cyclic_ring_chain_before_writing() {
        let (mut arena, mut terrain, particles, flyer_slot) = ring_fixture(8, 19);
        let group = construct_ring_group(&mut arena, &mut terrain, &particles, flyer_slot).unwrap();
        let first = group.rings[0].source_slot;
        put_word(&mut arena.entities[usize::from(first)], 56, first);

        let mut current_flyer = arena.entities[usize::from(flyer_slot)].clone();
        current_flyer.set_source_byte_46(2);
        put_word(&mut current_flyer, 44, 0x4567);
        set_owner(&mut current_flyer, 0x2345);
        arena.entities[usize::from(flyer_slot)] = current_flyer;
        let before = arena.entities[usize::from(first)].clone();

        assert!(propagate_current_flyer_to_ring_chain(&mut arena, flyer_slot, first).is_err());
        let after = &arena.entities[usize::from(first)];
        assert_eq!(after.record[74], before.record[74]);
        assert_eq!(word(after, 44), word(&before, 44));
        assert_eq!(after.source_id, before.source_id);
        assert_eq!(word(after, 30), word(&before, 30));
    }

    #[test]
    fn terminal_ring_dispatches_to_each_real_family_constructor_with_summon_provenance() {
        let campaign_runtime = campaign_runtime_fixture();
        for selector in [2_u8, 19, 25, 16] {
            let requested = usize::from(source_group_size(selector).unwrap());
            let (mut arena, mut terrain, particles, flyer_slot) =
                ring_fixture(requested + 1, selector);
            let group =
                construct_ring_group(&mut arena, &mut terrain, &particles, flyer_slot).unwrap();
            assert!(group.complete(), "family selector {selector}");
            let ring_slot = group.rings[0].source_slot;
            arena.entities[usize::from(ring_slot)].set_life(1);

            let mut runtime = campaign_runtime.clone();
            let cymmerian_counter_at = crate::native_world::RUNTIME_BYTES
                + usize::from(source_model(SummonArmyMember::Cymmerian));
            let initial_cymmerian_counter =
                (selector == 25).then(|| runtime.bytes()[cymmerian_counter_at]);
            let spawned = step_ring_with_family(
                &mut arena,
                &mut runtime,
                &mut terrain,
                &particles,
                ring_slot,
            )
            .unwrap();
            let RingStepOutcome::Spawned { creature } = spawned else {
                panic!("family selector {selector} did not allocate a creature: {spawned:?}");
            };
            assert_eq!(arena.economy.free_slot_count(), 0);
            let identity = arena
                .creature_runtime_identity(
                    creature.source_slot,
                    &std::collections::BTreeMap::new(),
                    &std::collections::BTreeMap::new(),
                )
                .expect("family constructor binds its live generation");
            assert_eq!(identity.handle, creature);
            assert_eq!(
                identity.origin,
                CreatureRootOrigin::SummonedSpell { holder_slot: 5 }
            );
            assert_eq!(identity.role, CreatureRuntimeRole::Root);

            if let Some(initial_counter) = initial_cymmerian_counter {
                let row = &arena.entities[usize::from(creature.source_slot)];
                assert_eq!(row.phase, initial_counter);
                assert_eq!(
                    runtime.bytes()[cymmerian_counter_at],
                    initial_counter.wrapping_add(1)
                );
            }

            if selector == 19 {
                let carrier = arena
                    .economy
                    .authored_class5()
                    .iter()
                    .find(|source| {
                        source.active && source.source_entity_index == creature.source_slot
                    })
                    .expect("Firefly has its typed class-5 carrier");
                let sidecar = arena
                    .economy
                    .authored_fireflies()
                    .iter()
                    .find(|state| state.source_entity_index == creature.source_slot)
                    .expect("Firefly has its authored-family sidecar");
                assert_eq!((carrier.model, carrier.action), (19, 159));
                assert_eq!(arena.economy.source_owner(creature.source_slot), 42);
                assert_eq!(sidecar.authored_index, None);
                assert_eq!(sidecar.parent_entity_index, 6);
                assert_eq!(sidecar.word_0x2e, SUMMONED_REMAINING_LIFETIME);
                assert_eq!(sidecar.stage_var2, SUMMONED_STAGE_MODE);
            }
        }
    }

    #[test]
    fn ring_birth_preserves_caster_generation_when_parent_slot_is_reused() {
        let source_runtime = campaign_runtime_fixture();
        for selector in [2_u8, 19, 25, 16] {
            // 0 keeps the caster, 1 reuses it before impact, and 2 reuses it
            // after ring creation but before the child is born.
            for reuse_phase in 0..=2 {
                let count = usize::from(source_group_size(selector).unwrap());
                let (mut arena, mut terrain, particles, flyer_slot) =
                    ring_fixture(count + 3, selector);
                let parent_slot = arena
                    .allocate_source_slot_with_terrain(&mut terrain)
                    .unwrap();
                let position = arena.entities[usize::from(flyer_slot)].position;
                let mut parent = super::super::cave_features::new_event(parent_slot, 5, position);
                parent.class = 3;
                parent.model = 0;
                parent.record[67] = 3;
                parent.set_life(1000);
                arena.entities[usize::from(parent_slot)] = parent;
                let parent_handle = arena.handle(parent_slot).unwrap();
                put_word(
                    &mut arena.entities[usize::from(flyer_slot)],
                    44,
                    parent_slot,
                );
                let flyer = arena.handle(flyer_slot).unwrap();
                assert!(arena.bind_creature_parent(flyer, parent_handle));
                let replace_parent = |arena: &mut SourceArena, terrain: &mut SessionTerrain| {
                    arena.economy.release_slot(parent_slot, None);
                    assert_eq!(
                        arena.allocate_source_slot_with_terrain(terrain),
                        Some(parent_slot)
                    );
                    let mut replacement =
                        super::super::cave_features::new_event(parent_slot, 6, position);
                    replacement.class = 3;
                    replacement.model = 0;
                    replacement.record[67] = 3;
                    replacement.set_life(1000);
                    arena.entities[usize::from(parent_slot)] = replacement;
                    assert_ne!(arena.handle(parent_slot).unwrap(), parent_handle);
                };
                if reuse_phase == 1 {
                    replace_parent(&mut arena, &mut terrain);
                }
                let group =
                    construct_ring_group(&mut arena, &mut terrain, &particles, flyer_slot).unwrap();
                assert!(group.complete());
                for ring in &group.rings {
                    assert_eq!(arena.creature_parent_binding(*ring), Some(parent_handle));
                }
                if reuse_phase == 2 {
                    replace_parent(&mut arena, &mut terrain);
                }
                let ring = group.rings[0];
                arena.entities[usize::from(ring.source_slot)].set_life(1);
                let mut runtime = source_runtime.clone();
                let RingStepOutcome::Spawned { creature } = step_ring_with_family(
                    &mut arena,
                    &mut runtime,
                    &mut terrain,
                    &particles,
                    ring.source_slot,
                )
                .unwrap() else {
                    panic!("selector {selector}, reuse phase {reuse_phase}: child not created");
                };
                assert_eq!(arena.creature_parent_binding(creature), Some(parent_handle));
                assert_eq!(
                    arena.creature_parent(creature),
                    (reuse_phase == 0).then_some(parent_handle)
                );
                if selector == 19 {
                    let state = arena
                        .economy
                        .authored_fireflies()
                        .iter()
                        .find(|state| state.source_entity_index == creature.source_slot)
                        .unwrap();
                    assert_eq!(state.parent_generation, parent_handle.generation);
                }
            }
        }
    }

    #[test]
    fn terminal_firefly_ring_reports_real_slot_exhaustion_without_fake_success() {
        let (mut arena, mut terrain, particles, flyer_slot) = ring_fixture(8, 19);
        let group = construct_ring_group(&mut arena, &mut terrain, &particles, flyer_slot).unwrap();
        assert!(group.complete());
        assert_eq!(arena.economy.free_slot_count(), 0);
        let ring_slot = group.rings[0].source_slot;
        arena.entities[usize::from(ring_slot)].set_life(1);

        let mut runtime = campaign_runtime_fixture();
        assert_eq!(
            step_ring_with_family(
                &mut arena,
                &mut runtime,
                &mut terrain,
                &particles,
                ring_slot,
            )
            .unwrap(),
            RingStepOutcome::NoCreatureSlot {
                member: SummonArmyMember::Firefly,
            }
        );
        assert_eq!(arena.economy.free_slot_count(), 0);
        assert!(
            !arena
                .economy
                .authored_class5()
                .iter()
                .any(|source| source.active && source.model == 19)
        );
        assert_eq!(arena.entities[usize::from(ring_slot)].life, 0);
    }
}
