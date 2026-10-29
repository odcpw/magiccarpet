//! Source row construction and topology for the model27 root, five heads and
//! forty-five body segments.

use crate::campaign::source_arena::{SourceArena, SourceEntityHandle, SourceEntityView};
use crate::native_world::{
    EntitySnapshot, MapType, ParticleParameter, RuntimeSnapshot, SessionTerrain,
};
use crate::{FlightWorld, Position, move_entity};

use super::{
    BODY_ACTION, BODY_PARTICLE, HEAD_ACTION, HEAD_PARTICLE, PROPERTIES, ROOT_DESCRIPTOR,
    ROOT_PARTICLE, SOURCE_MODEL,
};

const ENTITY_CLASS: u8 = 5;
const SOURCE_OWNER: usize = 54;
const SOURCE_NEXT: usize = 56;
const SOURCE_PART_INDEX: usize = 63;

const HEAD_POSES: [[i16; 5]; 5] = [
    [390, 20, 610, 9, 1771],
    [440, 110, 600, 407, 1685],
    [430, -100, 600, 1641, 1707],
    [420, 50, 450, 284, 1905],
    [420, -10, 450, 770, 1157],
];

pub(crate) fn head_angles(index: usize) -> (i16, i16) {
    (HEAD_POSES[index][3], HEAD_POSES[index][4])
}

const BODY_CURVE: [[i16; 2]; 16] = [
    [0, 0],
    [106, 36],
    [151, 51],
    [191, 65],
    [220, 75],
    [246, 84],
    [275, 94],
    [297, 102],
    [318, 109],
    [338, 116],
    [361, 124],
    [380, 130],
    [398, 136],
    [416, 143],
    [437, 150],
    [454, 156],
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Chain {
    pub(crate) root: SourceEntityHandle,
    pub(crate) heads: [SourceEntityHandle; 5],
    pub(crate) bodies: [[SourceEntityHandle; 9]; 5],
    pub(crate) ordered: Vec<SourceEntityHandle>,
}

fn word(row: &EntitySnapshot, offset: usize) -> u16 {
    u16::from_le_bytes([row.record[offset], row.record[offset + 1]])
}

fn put_word(row: &mut EntitySnapshot, offset: usize, value: u16) {
    row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn constructor_phase(runtime: &RuntimeSnapshot) -> Result<u8, String> {
    runtime
        .bytes
        .get(crate::native_world::RUNTIME_BYTES + usize::from(SOURCE_MODEL))
        .copied()
        .ok_or_else(|| "model27 constructor counter table is absent".to_owned())
}

fn prepare_row(
    slot: u16,
    seed: u16,
    root_slot: u16,
    part_index: u8,
    action: u8,
    position: Position,
    phase: Option<u8>,
    particle_index: u16,
    descriptor: Option<u16>,
    particles: &[ParticleParameter],
) -> Result<EntitySnapshot, String> {
    let mut row = crate::world::terrain::cave_features::new_event(slot, seed, position);
    row.class = ENTITY_CLASS;
    row.model = SOURCE_MODEL;
    row.source_id = root_slot;
    row.record[30..32].copy_from_slice(&root_slot.to_le_bytes());
    row.record[67] = ENTITY_CLASS;
    row.record[68] = SOURCE_MODEL;
    row.byte_3b = part_index;
    row.record[SOURCE_PART_INDEX] = part_index;
    row.set_source_action(action);
    if let Some(phase) = phase {
        row.set_phase(phase);
    }
    put_word(&mut row, SOURCE_OWNER, root_slot);
    row.parameter_relation = descriptor;
    if let Some(descriptor) = descriptor {
        row.record[164..168].copy_from_slice(&(u32::from(descriptor) + 1).to_le_bytes());
    }
    crate::world::effects::bind_particle(&mut row, particle_index, particles)?;
    Ok(row)
}

pub(crate) fn initialize_root(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    slot: u16,
    position: Position,
    particles: &[ParticleParameter],
) -> Result<(), String> {
    let phase = constructor_phase(runtime)?;
    let seed = arena.new_event_rng(slot);
    let mut root = prepare_row(
        slot,
        seed,
        slot,
        5,
        super::ACTION_BASE + 1,
        position,
        Some(phase),
        ROOT_PARTICLE,
        Some(ROOT_DESCRIPTOR),
        particles,
    )?;
    root.set_yaw(0);
    root.set_pitch(0);
    root.set_source_roll(0);
    root.set_fov(0);
    root.set_actual_speed(PROPERTIES.root_initial_speed);
    put_word(&mut root, 136, PROPERTIES.root_minimum_speed as u16);
    put_word(&mut root, 138, PROPERTIES.root_maximum_speed as u16);
    root.set_max_life(PROPERTIES.root_max_life);
    root.set_life(PROPERTIES.root_initial_life);
    root.set_mana(PROPERTIES.root_initial_mana);
    root.record[69] = 3;
    root.record[60] = 1;
    root.set_source_dword_10(i32::try_from(slot % 100).expect("slot modulo 100 fits i32"));
    // sub_2AC50 enables the source creature and sets shift pitch/roll.
    root.set_control_flags(root.control_flags | 0x8000_0000);
    put_word(&mut root, 88, 1024);
    put_word(&mut root, 90, 1536);
    root.record[61] = PROPERTIES.root_initial_source_cadence;
    root.byte_3b = 5;
    root.record[63] = 5;
    arena.entities[usize::from(slot)] = root;
    arena.economy.source_rng.insert(slot, seed);
    Ok(())
}

/// sub_2AC50 is reached only after the complete 51-row allocation succeeds.
/// Commit its one model counter increment at that point; segment constructors
/// do not consume counter27.
pub(crate) fn commit_root_constructor_phase(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    root_slot: u16,
) -> Result<(), String> {
    let counter = runtime
        .bytes
        .get_mut(crate::native_world::RUNTIME_BYTES + usize::from(SOURCE_MODEL))
        .ok_or_else(|| "model27 constructor counter table is absent".to_owned())?;
    let phase = *counter;
    *counter = counter.wrapping_add(1);
    let root = arena
        .entities
        .get_mut(usize::from(root_slot))
        .filter(|row| row.active && row.class == ENTITY_CLASS && row.model == SOURCE_MODEL)
        .ok_or_else(|| format!("model27 root {root_slot} lost its constructor row"))?;
    root.set_phase(phase);
    Ok(())
}

pub(crate) fn initialize_head(
    arena: &mut SourceArena,
    root_slot: u16,
    slot: u16,
    head_index: u8,
    position: Position,
    particles: &[ParticleParameter],
) -> Result<(), String> {
    let seed = arena.new_event_rng(slot);
    let mut head = prepare_row(
        slot,
        seed,
        root_slot,
        head_index,
        HEAD_ACTION,
        position,
        None,
        HEAD_PARTICLE,
        Some(super::HEAD_DESCRIPTOR),
        particles,
    )?;
    let rng = crate::native_world::level2_next_rng16(seed);
    head.set_entity_rng(rng);
    head.set_source_roll((rng & 0x07ff) as i16);
    let rng = crate::native_world::level2_next_rng16(rng);
    head.set_fov((rng & 0x07ff) as i16);
    head.set_actual_speed(PROPERTIES.head_initial_speed);
    put_word(&mut head, 136, PROPERTIES.head_initial_speed as u16);
    head.set_life(
        PROPERTIES
            .head_base_life
            .wrapping_add(PROPERTIES.head_life_per_index * (i32::from(head_index) * 10 + 1)),
    );
    head.set_max_life(head.life);
    head.record[60] = 1;
    head.set_control_flags(head.control_flags | 0xa000_0000);
    arena.economy.source_rng.insert(slot, head.entity_rng);
    arena.entities[usize::from(slot)] = head;
    Ok(())
}

pub(crate) fn initialize_body(
    arena: &mut SourceArena,
    root_slot: u16,
    slot: u16,
    head_index: u8,
    position: Position,
    particles: &[ParticleParameter],
) -> Result<(), String> {
    let seed = arena.new_event_rng(slot);
    let mut body = prepare_row(
        slot,
        seed,
        root_slot,
        head_index,
        BODY_ACTION,
        position,
        None,
        BODY_PARTICLE,
        None,
        particles,
    )?;
    body.set_control_flags(body.control_flags | 0xa000_0000);
    arena.economy.source_rng.insert(slot, seed);
    arena.entities[usize::from(slot)] = body;
    Ok(())
}

fn part(
    arena: &SourceArena,
    slot: u16,
    root: u16,
    action: u8,
    index: u8,
) -> Option<SourceEntityHandle> {
    let row = arena.entities.get(usize::from(slot))?;
    (row.active
        && row.class == ENTITY_CLASS
        && row.model == SOURCE_MODEL
        && row.action == action
        && row.record[SOURCE_PART_INDEX] == index
        && row.source_id == root
        && word(row, 30) == root
        && word(row, SOURCE_OWNER) == root)
        .then(|| arena.handle(slot))
        .flatten()
}

/// Resolve the complete live root->head->body chain, rejecting generation
/// replacement, a missing segment, an unexpected callback action or a cycle.
pub(crate) fn read_chain(arena: &SourceArena, root: SourceEntityHandle) -> Result<Chain, String> {
    let Some(SourceEntityView::Imported(root_row)) = arena.resolve(root) else {
        return Err(format!("model27 root {} is stale", root.source_slot));
    };
    if root_row.class != ENTITY_CLASS || root_row.model != SOURCE_MODEL || root_row.action == 180 {
        return Err(format!(
            "model27 root {} has the wrong source identity",
            root.source_slot
        ));
    }
    if root_row.drawing_disabled() {
        return Err(format!("model27 root {} is hidden", root.source_slot));
    }

    let killed_chain = root_row.action == super::ACTION_BASE + 5;
    let mut current = word(root_row, SOURCE_NEXT);
    let mut ordered = Vec::with_capacity(PROPERTIES.roots_require_free_slots - 1);
    let mut heads = Vec::with_capacity(PROPERTIES.head_count);
    let mut bodies = Vec::with_capacity(PROPERTIES.head_count);
    for head_index in 0..PROPERTIES.head_count {
        let head_slot = current;
        let Some(head) = part(
            arena,
            head_slot,
            root.source_slot,
            if killed_chain {
                super::ACTION_BASE + 5
            } else {
                HEAD_ACTION
            },
            head_index as u8,
        ) else {
            return Err(format!(
                "model27 root {} lost head {head_index}",
                root.source_slot
            ));
        };
        if ordered.contains(&head) {
            return Err(format!(
                "model27 root {} has a cyclic part chain",
                root.source_slot
            ));
        }
        ordered.push(head);
        heads.push(head);
        current = word(&arena.entities[usize::from(head_slot)], SOURCE_NEXT);

        let mut group = Vec::with_capacity(PROPERTIES.body_parts_per_head);
        for body_index in 0..PROPERTIES.body_parts_per_head {
            let body_slot = current;
            let Some(body) = part(
                arena,
                body_slot,
                root.source_slot,
                if killed_chain {
                    super::ACTION_BASE + 5
                } else {
                    BODY_ACTION
                },
                head_index as u8,
            ) else {
                return Err(format!(
                    "model27 root {} lost body segment {body_index} for head {head_index}",
                    root.source_slot
                ));
            };
            if ordered.contains(&body) {
                return Err(format!(
                    "model27 root {} has a cyclic part chain",
                    root.source_slot
                ));
            }
            ordered.push(body);
            group.push(body);
            current = word(&arena.entities[usize::from(body_slot)], SOURCE_NEXT);
        }
        bodies.push(group);
    }
    if current != 0 {
        return Err(format!(
            "model27 root {} has more than 50 parts",
            root.source_slot
        ));
    }
    let heads: [SourceEntityHandle; 5] = heads
        .try_into()
        .map_err(|_| "model27 constructor did not retain five heads".to_owned())?;
    let bodies: [[SourceEntityHandle; 9]; 5] = bodies
        .into_iter()
        .map(|group| {
            group
                .try_into()
                .map_err(|_| "model27 constructor did not retain nine body parts".to_owned())
        })
        .collect::<Result<Vec<_>, String>>()?
        .try_into()
        .map_err(|_| "model27 constructor did not retain five body groups".to_owned())?;
    Ok(Chain {
        root,
        heads,
        bodies,
        ordered,
    })
}

pub(crate) fn hide_chain(arena: &mut SourceArena, chain: &Chain) {
    for handle in std::iter::once(chain.root).chain(chain.ordered.iter().copied()) {
        if arena.slot_generation(handle.source_slot) == handle.generation
            && let Some(row) = arena.entities.get_mut(usize::from(handle.source_slot))
            && row.active
        {
            row.disable_drawing();
        }
    }
}

fn put_position(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    handle: SourceEntityHandle,
    position: Position,
) -> Result<(), String> {
    if arena.slot_generation(handle.source_slot) != handle.generation {
        return Err(format!(
            "model27 part {} was reused during its callback",
            handle.source_slot
        ));
    }
    let Some(row) = arena.entities.get_mut(usize::from(handle.source_slot)) else {
        return Err(format!(
            "model27 part {} is outside the source arena",
            handle.source_slot
        ));
    };
    if !row.active || row.class != ENTITY_CLASS || row.model != SOURCE_MODEL {
        return Err(format!(
            "model27 part {} lost its source row",
            handle.source_slot
        ));
    }
    row.set_position(position);
    crate::world::effects::sync_position(arena, terrain, handle.source_slot, position, true, false)
}

pub(crate) fn distance_3d(left: Position, right: Position) -> i32 {
    let dx = i64::from(left.x.wrapping_sub(right.x) as i16);
    let dy = i64::from(left.y.wrapping_sub(right.y) as i16);
    let dz = i64::from(left.z.wrapping_sub(right.z) as i16);
    integer_sqrt((dx * dx + dy * dy + dz * dz) as u64) as i32
}

fn integer_sqrt(value: u64) -> u64 {
    if value < 2 {
        return value;
    }
    let mut estimate = value;
    let mut next = estimate.div_ceil(2);
    while next < estimate {
        estimate = next;
        next = (estimate + value / estimate) / 2;
    }
    estimate
}

fn curve_index(anchor: Position, head: Position) -> usize {
    let source_step = (distance_3d(anchor, head) - 468) / 24;
    usize::try_from((16 - source_step).clamp(0, 15)).expect("curve index is clamped")
}

fn segment_pitch_offset(index: usize, curve: usize) -> i16 {
    match index {
        0 => 0,
        1 | 8 => -BODY_CURVE[curve][0],
        2 | 7 => -BODY_CURVE[curve][1],
        3 | 6 => BODY_CURVE[curve][1],
        4 | 5 => BODY_CURVE[curve][0],
        _ => unreachable!("model27 has exactly nine body segments"),
    }
}

fn root_head_anchor(root: &EntitySnapshot, head_index: usize) -> Position {
    let [forward, yaw_offset, vertical, _, _] = HEAD_POSES[head_index];
    let mut anchor = root.position;
    move_entity(&mut anchor, root.yaw.wrapping_add(yaw_offset), 0, forward);
    anchor.z = anchor.z.wrapping_add(vertical);
    anchor
}

pub(crate) fn pose_head(root: &EntitySnapshot, head_index: usize) -> Position {
    let mut position = root_head_anchor(root, head_index);
    let [_, _, _, yaw_offset, pitch_offset] = HEAD_POSES[head_index];
    move_entity(
        &mut position,
        root.yaw.wrapping_add(yaw_offset),
        root.pitch.wrapping_add(pitch_offset),
        672,
    );
    position
}

/// Source sub_2AA90: rebuild each nine-link body from its root anchor and the
/// corresponding manually advanced head. Children do not run callbacks.
pub(crate) fn pose_bodies(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    chain: &Chain,
    only_head: Option<usize>,
) -> Result<(), String> {
    let root = arena
        .resolve(chain.root)
        .and_then(|view| match view {
            SourceEntityView::Imported(row) => Some(row.clone()),
            SourceEntityView::Dynamic(_) => None,
        })
        .ok_or_else(|| format!("model27 root {} changed generation", chain.root.source_slot))?;
    for head_index in 0..PROPERTIES.head_count {
        if only_head.is_some_and(|index| index != head_index) {
            continue;
        }
        let head_handle = chain.heads[head_index];
        let head = arena
            .resolve(head_handle)
            .and_then(|view| match view {
                SourceEntityView::Imported(row) => Some(row.clone()),
                SourceEntityView::Dynamic(_) => None,
            })
            .ok_or_else(|| format!("model27 head {head_index} changed generation"))?;
        // sub_29A90 calls sub_2AA90 only for final head states 0..7
        // and 11..12; hidden/regrowing states 8..10 and 13..15 do not pose.
        if !matches!(head.record[74], 0..=7 | 11..=12) {
            continue;
        }
        let mut position = root_head_anchor(&root, head_index);
        let curve = curve_index(position, head.position);
        let (yaw, pitch) =
            crate::level1_mission::projectile_guidance_angles(position, head.position);
        for segment_index in 0..PROPERTIES.body_parts_per_head {
            if segment_index != 0 {
                move_entity(
                    &mut position,
                    yaw,
                    pitch.wrapping_add(segment_pitch_offset(segment_index, curve)),
                    PROPERTIES.body_spacing,
                );
            }
            let body = chain.bodies[head_index][segment_index];
            let mut body_position = position;
            if head.record[74] == 7 && head.record[72] == 8 {
                let ground = terrain.ground_height(body_position);
                if body_position.z <= ground {
                    body_position.z = ground;
                }
            }
            put_position(arena, terrain, body, body_position)?;
        }
    }
    Ok(())
}

/// sub_2AD40 moves each head once from the constructor position. The nine
/// bodies remain at their allocated position until the first 29A90 tail.
pub(crate) fn pose_new_chain(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    chain: &Chain,
) -> Result<(), String> {
    let root = arena
        .resolve(chain.root)
        .and_then(|view| match view {
            SourceEntityView::Imported(row) => Some(row.clone()),
            SourceEntityView::Dynamic(_) => None,
        })
        .ok_or_else(|| format!("model27 root {} changed generation", chain.root.source_slot))?;
    for head_index in 0..PROPERTIES.head_count {
        let head = chain.heads[head_index];
        let mut row = arena.entities[usize::from(head.source_slot)].clone();
        super::combat::move_attached_head(&mut row, &root);
        put_position(arena, terrain, head, row.position)?;
    }
    Ok(())
}

pub(crate) fn hide_unregistered(arena: &mut SourceArena, handles: &[SourceEntityHandle]) {
    for handle in handles {
        if arena.slot_generation(handle.source_slot) == handle.generation
            && let Some(row) = arena.entities.get_mut(usize::from(handle.source_slot))
            && row.active
        {
            row.disable_drawing();
        }
    }
}

pub(crate) fn map_type_allows_model27(terrain: &SessionTerrain) -> bool {
    terrain.map_type() != MapType::Cave
}
