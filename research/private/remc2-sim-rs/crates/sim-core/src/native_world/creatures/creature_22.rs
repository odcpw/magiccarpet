//! Paired, colour-changing creature: source constructor 4CA00 and actions 176–183.
//!
//! `Actor` is a temporary typed view, decoded and committed by the MC2 arena
//! adapter. The source row remains the sole mutable carrier, including links,
//! hit/possession mailboxes and draw-consumed animation state.

use crate::campaign::source_arena::SourceEntityHandle;
use crate::level_definition::CREATURE22_CLASS_PROPERTIES as PROPERTIES;
use crate::native_world::{ENTITY_DRAW_DISABLED_BIT, SessionTerrain, level2_next_rng16};
use crate::{FlightWorld, Position, Tile, heading_for_positions};

pub(crate) mod mc2;
mod stage;
#[cfg(test)]
mod tests;

pub(crate) use super::archer::{
    ArcherStageDescriptor as StageDescriptor, ArcherStageState as StageState,
};

pub(crate) const SOURCE_MODEL: u8 = 22;
pub(crate) const SOURCE_DESCRIPTOR: u16 = 90;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Allocation {
    pub handle: SourceEntityHandle,
    pub rng: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Actor {
    pub handle: SourceEntityHandle,
    pub source_id: u16,
    pub entity_rng: u16,
    pub control_flags: u32,
    pub action: u8,
    pub position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub speed: i16,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub life: i32,
    pub max_life: i32,
    pub mana: i32,
    pub max_mana: i32,
    pub phase: u8,
    pub cadence_counter: u8,
    pub animation_frame: u8,
    /// Root: authored odd length. Child: signed position in the paired chain.
    pub length_or_ordinal: i8,
    pub rotation: i16,
    pub subspell_index: u16,
    pub oscillation_flags: u16,
    pub dword10: i32,
    /// Child word96, generation-bound by the adapter. Root word96 is the
    /// growth countdown / StageVar target and lives in `target_or_growth`.
    pub root: Option<SourceEntityHandle>,
    pub previous: Option<SourceEntityHandle>,
    pub next: Option<SourceEntityHandle>,
    pub target_or_growth: u16,
    pub owner: u16,
    pub damage: i32,
    pub hit_source: u16,
    pub possessor: u16,
    pub spell_amount: i32,
    pub attacker: u16,
    pub visual_index: u16,
    pub half_width: i16,
    pub height: i16,
    pub visual_radius: i16,
    /// Source word24: altitude hold in ordinary callbacks, killer in StageVar.
    pub climb_hold: i16,
    pub stage: StageState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Target {
    pub slot: u16,
    pub source_id: u16,
    pub class: u8,
    pub model: u8,
    pub life: i32,
    pub control_flags: u32,
    pub position: Position,
    pub yaw: i16,
    pub roll: i16,
    pub half_width: i16,
    pub target: u16,
    pub attacker: u16,
    pub parent: u16,
    pub stage_mode: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PlayerView {
    pub target: Target,
    pub castle_slot: u16,
    pub castle_position: Position,
    pub castle_mana: i32,
    pub castle_max_mana: i32,
}

/// Narrow live arena operations. No callback may retain a mutable actor copy
/// across a call which can allocate/reuse its address. Writes validate handles.
pub(crate) trait Runtime {
    fn actor(&self, handle: SourceEntityHandle) -> Option<Actor>;
    fn actor_at(&self, slot: u16) -> Option<Actor>;
    fn write_actor(&mut self, actor: Actor);
    fn allocate(&mut self) -> Option<Allocation>;
    fn hide_uninitialized(&mut self, allocation: Allocation);
    /// Source callback pointers still address a newborn occupant after
    /// NewEvent replacement. This explicit operation is intentionally raw.
    fn hide_current_slot(&mut self, slot: u16);
    fn terrain(&self) -> &SessionTerrain;
    fn frozen_model_slots(&self, model: u8) -> &[u16];
    fn frozen_class3_slots(&self) -> &[u16];
    fn target(&self, slot: u16) -> Option<Target>;
    fn player_colour_base(&self, slot: u16) -> Option<u16>;
    fn player(&mut self, slot: u16) -> Option<PlayerView>;
    fn add_castle_mana(&mut self, castle_slot: u16, source: Actor);
    fn sprite_rotation(&self, visual_index: u16) -> i16;
    fn mana_sphere(&mut self, actor: Actor);
    fn sound(&mut self, slot: u16, sample: u8);
    fn stage_descriptor(&self, index: u8) -> Option<StageDescriptor>;
    fn bind_stage_descriptor(&mut self, index: u8) -> Option<StageDescriptor>;
    fn rearm_model_relations(&mut self);
    fn emit_effect(&mut self, position: Position, model: u8, source_slot: u16);
    fn feedback(&mut self, player: u16, message: u8);
    fn decrement_population(&mut self);
}

impl Actor {
    /// Bare 4CA00. The caller has already enforced 15 free ordinary slots;
    /// authored par1/body construction is deliberately a separate operation.
    pub(crate) fn construct(
        allocation: Allocation,
        phase: u8,
        position: Position,
        terrain: &SessionTerrain,
    ) -> Self {
        let entity_rng = level2_next_rng16(allocation.rng);
        let heading = (entity_rng & 2047).wrapping_sub(1) as i16;
        // 4CA00 places the constructor through the shared source proposal.
        let position = Position {
            z: terrain
                .ground_height(position)
                .wrapping_add(PROPERTIES.ground_clearance),
            ..position
        };
        terrain.set_source_predicted_position(position);
        Self {
            handle: allocation.handle,
            source_id: allocation.handle.source_slot,
            entity_rng,
            control_flags: 8,
            action: 176,
            position,
            yaw: heading,
            pitch: heading,
            roll: heading,
            speed: PROPERTIES.maximum_speed,
            minimum_speed: PROPERTIES.minimum_speed,
            maximum_speed: PROPERTIES.maximum_speed,
            life: PROPERTIES.max_life,
            max_life: PROPERTIES.max_life,
            mana: PROPERTIES.initial_mana,
            max_mana: 0,
            phase,
            cadence_counter: PROPERTIES
                .scan_period
                .wrapping_sub(phase % PROPERTIES.scan_period)
                .wrapping_add(4),
            animation_frame: 0,
            length_or_ordinal: PROPERTIES.default_length,
            rotation: PROPERTIES.minimum_rotation,
            subspell_index: 0,
            oscillation_flags: 0,
            dword10: 0,
            root: None,
            previous: None,
            next: None,
            target_or_growth: PROPERTIES.growth_period,
            owner: 0,
            damage: 0,
            hit_source: 0,
            possessor: 0,
            spell_amount: 0,
            attacker: 0,
            visual_index: 0,
            half_width: 0,
            height: 0,
            visual_radius: 0,
            climb_hold: 0,
            stage: StageState::default(),
        }
    }

    pub(crate) fn hidden(self) -> bool {
        self.control_flags & ENTITY_DRAW_DISABLED_BIT != 0
    }
    pub(crate) fn hide(&mut self) {
        self.control_flags |= ENTITY_DRAW_DISABLED_BIT;
    }
}

/// Source action182 has no dispatch case and the descriptor's bit8 is clear.
/// The scheduler must never convert that route into a successful empty step.
pub(crate) const fn enabled(action: u8) -> bool {
    matches!(action, 176..=181 | 183)
}

/// sub_4CB60, after authored par1 has replaced the bare constructor length.
pub(crate) fn initialize_body(handle: SourceEntityHandle, runtime: &mut dyn Runtime) {
    let Some(root) = runtime.actor(handle) else {
        return;
    };
    let mut previous = root.handle;
    for ordinal in 1..=i16::from(root.length_or_ordinal) / 2 {
        for ordinal in [ordinal as i8, -(ordinal as i8)] {
            let Some(allocation) = runtime.allocate() else {
                return;
            };
            if !link_segment(handle, previous, allocation, ordinal, runtime) {
                return;
            }
            previous = allocation.handle;
        }
    }
    recolour_and_scale(handle, runtime);
    for child in chain(handle, runtime).into_iter().skip(1) {
        position_segment(child, runtime);
    }
}

fn chain(handle: SourceEntityHandle, runtime: &dyn Runtime) -> Vec<SourceEntityHandle> {
    let mut result = Vec::new();
    let mut current = Some(handle);
    while let Some(handle) = current {
        if result.contains(&handle) || result.len() >= 1000 {
            break;
        }
        let Some(actor) = runtime.actor(handle) else {
            break;
        };
        result.push(handle);
        current = actor.next;
    }
    result
}

fn link_segment(
    root: SourceEntityHandle,
    previous: SourceEntityHandle,
    allocation: Allocation,
    ordinal: i8,
    runtime: &mut dyn Runtime,
) -> bool {
    let (Some(root_actor), Some(mut predecessor)) = (runtime.actor(root), runtime.actor(previous))
    else {
        runtime.hide_uninitialized(allocation);
        return false;
    };
    // sub274C0 copies the predecessor, including its RNG and source ID. It
    // does not keep NewEvent's seed or assign an independent creature ID.
    let mut child = predecessor;
    child.handle = allocation.handle;
    child.previous = Some(previous);
    child.next = None;
    child.root = Some(root);
    child.target_or_growth = root.source_slot;
    child.phase = ordinal.unsigned_abs() & 1;
    child.control_flags &= !4;
    child.length_or_ordinal = ordinal;
    child.action = 180;
    child.rotation = 0;
    child.owner = 0;
    child.mana = 0;
    child.position = root_actor.position;
    child.life = child.max_life;
    predecessor.next = Some(allocation.handle);
    runtime.write_actor(predecessor);
    // 274C0 places each newly linked segment through predictedAxis.
    runtime.terrain().set_source_predicted_position(root_actor.position);
    runtime.write_actor(child);
    true
}

const COLOUR_OFFSETS: [[u8; 8]; 8] = [
    [0, 0, 0, 0, 0, 0, 0, 0],
    [1, 0, 0, 0, 0, 0, 0, 0],
    [2, 1, 0, 0, 0, 0, 0, 0],
    [3, 2, 1, 0, 0, 0, 0, 0],
    [4, 3, 2, 1, 0, 0, 0, 0],
    [5, 4, 3, 3, 1, 0, 0, 0],
    [6, 5, 4, 3, 2, 1, 0, 0],
    [7, 6, 5, 4, 3, 2, 1, 0],
];

fn colour_index(base: u16, length: i8, ordinal: i8) -> Option<u16> {
    COLOUR_OFFSETS
        .get(usize::try_from(length / 2).ok()?)?
        .get(usize::from(ordinal.unsigned_abs()))
        .map(|offset| base + u16::from(*offset))
}

fn colour_base(owner: u16, runtime: &dyn Runtime) -> u16 {
    runtime.player_colour_base(owner).unwrap_or(52)
}

fn set_visual(actor: &mut Actor, visual: u16, runtime: &dyn Runtime) {
    actor.visual_index = visual;
    actor.visual_radius = runtime.sprite_rotation(visual) / 2;
}

fn recolour_and_scale(handle: SourceEntityHandle, runtime: &mut dyn Runtime) {
    let Some(root) = runtime.actor(handle) else {
        return;
    };
    let base = colour_base(root.owner, runtime);
    for handle in chain(handle, runtime) {
        let Some(mut actor) = runtime.actor(handle) else {
            continue;
        };
        let ordinal = if handle == root.handle {
            0
        } else {
            actor.length_or_ordinal
        };
        if let Some(visual) = colour_index(base, root.length_or_ordinal, ordinal) {
            set_visual(&mut actor, visual, runtime);
            let size = (PROPERTIES.scale_per_mille * i32::from(runtime.sprite_rotation(visual))
                / 1000) as i16;
            actor.half_width = size;
            actor.height = size;
            runtime.write_actor(actor);
        }
    }
}

/// Dynamic growth allocates a complete pair before attaching either row.
/// A failed second allocation preserves the first allocation hide-marked.
pub(crate) fn resize(handle: SourceEntityHandle, requested: i16, runtime: &mut dyn Runtime) {
    let requested = requested | 1;
    let Some(root) = runtime.actor(handle) else {
        return;
    };
    if requested < 1
        || requested > i16::from(PROPERTIES.maximum_length)
        || requested == i16::from(root.length_or_ordinal)
    {
        return;
    }
    let handles = chain(handle, runtime);
    let Some(mut tail) = handles.last().copied() else {
        return;
    };
    if i16::from(root.length_or_ordinal) >= requested {
        for _ in 0..(i16::from(root.length_or_ordinal) - requested) / 2 {
            let Some(last) = runtime.actor(tail) else {
                return;
            };
            let Some(penultimate) = last.previous.and_then(|handle| runtime.actor(handle)) else {
                return;
            };
            let Some(mut predecessor) = penultimate
                .previous
                .and_then(|handle| runtime.actor(handle))
            else {
                return;
            };
            predecessor.next = None;
            runtime.write_actor(predecessor);
            let mut penultimate = penultimate;
            let mut last = last;
            penultimate.hide();
            last.hide();
            runtime.write_actor(penultimate);
            runtime.write_actor(last);
            tail = predecessor.handle;
        }
    } else {
        let Some(first) = runtime.allocate() else {
            return;
        };
        let Some(second) = runtime.allocate() else {
            runtime.hide_uninitialized(first);
            return;
        };
        let Some(previous) = runtime.actor(tail) else {
            runtime.hide_uninitialized(first);
            runtime.hide_uninitialized(second);
            return;
        };
        // The source uses the tail byte even when the root is the tail.
        // Growing a one-row body therefore produces ordinals +2/-2.
        let ordinal = previous.length_or_ordinal.unsigned_abs().wrapping_add(1) as i8;
        if !link_segment(handle, tail, first, ordinal, runtime) {
            runtime.hide_uninitialized(second);
            return;
        }
        if !link_segment(handle, first.handle, second, -ordinal, runtime) {
            return;
        }
    }
    if let Some(mut root) = runtime.actor(handle) {
        root.length_or_ordinal = requested as i8;
        runtime.write_actor(root);
        recolour_and_scale(handle, runtime);
    }
}

/// One enabled action callback. The shared scheduler owns byte3E epilogue and
/// the next-pass reclaim; this function never increments phase itself.
pub(crate) fn step(handle: SourceEntityHandle, runtime: &mut dyn Runtime) -> bool {
    let Some(actor) = runtime.actor(handle) else {
        return false;
    };
    match actor.action {
        176 => {
            move_root(handle, runtime);
            animate(handle, runtime);
            possess_root(handle, runtime);
            grow(handle, runtime);
        }
        177 => {
            move_root(handle, runtime);
            animate(handle, runtime);
            propagate_colour(handle, runtime);
        }
        178 => {
            move_root(handle, runtime);
            animate(handle, runtime);
            possess_root(handle, runtime);
            grow(handle, runtime);
            return_to_castle(handle, runtime);
        }
        179 => {
            animate(handle, runtime);
            shrink_into_castle(handle, runtime);
        }
        180 => {
            position_segment(handle, runtime);
            interact_segment(handle, runtime);
        }
        181 => {
            for handle in chain(handle, runtime)
                .into_iter()
                .skip(1)
                .chain(std::iter::once(handle))
            {
                if let Some(actor) = runtime.actor(handle) {
                    runtime.mana_sphere(actor);
                }
                runtime.hide_current_slot(handle.source_slot);
            }
        }
        183 => stage::step(handle, runtime),
        _ => return false,
    }
    true
}

fn slope(position: Position, terrain: &SessionTerrain) -> i32 {
    let x = position.x.to_be_bytes()[0];
    let y = position.y.to_be_bytes()[0];
    let height = |x, y| {
        i32::from(terrain.ground_height(Position {
            x: u16::from(x) << 8,
            y: u16::from(y) << 8,
            z: 0,
        })) >> 5
    };
    let (a, b, c, d) = (
        height(x, y),
        height(x.wrapping_add(1), y),
        height(x.wrapping_add(1), y.wrapping_add(1)),
        height(x, y.wrapping_add(1)),
    );
    (a + d - b - c).abs().max((a + b - d - c).abs())
}

/// Descriptor90 has no ground settle offset/descent; it flies horizontally.
fn generic_move(actor: &mut Actor, terrain: &SessionTerrain) -> bool {
    if actor.control_flags & 0x800 != 0 {
        actor.control_flags &= !0x800;
        return false;
    }
    let original = actor.position;
    let yaw = actor.yaw as u16;
    let third = (yaw.wrapping_sub(85) & 255) | (yaw.wrapping_sub(341) & 0x700);
    // Preserve the source expression's byte-level retry, also used by Archer.
    let fourth = (yaw.wrapping_add(1024) & 0x700) | (yaw & 255);
    for (attempt, heading) in [yaw & 2047, yaw.wrapping_add(341) & 2047, third, fourth]
        .into_iter()
        .enumerate()
    {
        actor.yaw = heading as i16;
        let mut candidate = original;
        candidate.z = candidate.z.max(terrain.ground_height(candidate));
        crate::move_entity(&mut candidate, actor.yaw, 0, actor.speed);
        // sub_1B8C0 leaves even a rejected proposal in the shared source
        // scratch; 0x800 returns above without touching it.
        terrain.set_source_predicted_position(candidate);
        let crosses = (candidate.x >> 8, candidate.y >> 8) != (original.x >> 8, original.y >> 8);
        let mut blocked = false;
        if crosses || attempt != 0 {
            let mut probe = candidate;
            let mut distance = 0;
            while distance <= i32::from(actor.half_width.max(0)) {
                blocked = terrain.material_mask(probe) & !PROPERTIES.terrain_mask != 0
                    || terrain.cave_collision_blocked(probe)
                    || terrain.ceiling_height(probe).is_some_and(|ceiling| {
                        terrain.angle_flags(Tile {
                            x: probe.x.to_be_bytes()[0],
                            y: probe.y.to_be_bytes()[0],
                        }) & 8
                            != 0
                            || i32::from(terrain.ground_height(probe)) + i32::from(actor.height)
                                > i32::from(ceiling)
                    });
                if blocked {
                    break;
                }
                crate::move_entity(&mut probe, actor.yaw, 0, 256);
                distance += 256;
            }
            blocked |= slope(candidate, terrain) >= PROPERTIES.maximum_slope;
        }
        if !blocked {
            actor.position = candidate;
            actor.yaw = actor.yaw.wrapping_add(crate::turn_toward(
                actor.yaw,
                actor.roll,
                PROPERTIES.turn_limit,
            )) & 2047;
            if attempt == 0 {
                actor.control_flags &= !0x40000;
            }
            return false;
        }
        actor.control_flags |= 0x40000;
    }
    actor.life = -1;
    true
}

fn move_root(handle: SourceEntityHandle, runtime: &mut dyn Runtime) {
    let Some(mut actor) = runtime.actor(handle) else {
        return;
    };
    if actor.speed > actor.maximum_speed {
        actor.speed = actor.speed.wrapping_sub(2);
    }
    let old_width = actor.half_width;
    actor.half_width = i16::from(actor.length_or_ordinal) << 8;
    if generic_move(&mut actor, runtime.terrain()) {
        runtime.decrement_population();
    }
    actor.half_width = old_width;
    if actor.phase & 15 == 0 {
        for slot in runtime.frozen_model_slots(SOURCE_MODEL) {
            if let Some(peer) = runtime.actor_at(*slot)
                && peer.source_id != actor.source_id
                && (i32::from(actor.position.x as i16) - i32::from(peer.position.x as i16)).abs()
                    < i32::from(actor.half_width) * 2
                && (i32::from(actor.position.y as i16) - i32::from(peer.position.y as i16)).abs()
                    < i32::from(actor.half_width) * 2
                && (i32::from(actor.position.z) - i32::from(peer.position.z)).abs()
                    < i32::from(actor.height) * 2 + 32
                && actor.position.z >= peer.position.z
            {
                actor.position.z = actor.position.z.wrapping_add(64);
            }
        }
    }
    let mut maximum = 0i16;
    let mut highest = actor.position;
    // The current root copy has moved; children still hold their actual
    // preceding callback positions until their ascending source visits.
    for position in std::iter::once(actor.position).chain(
        chain(handle, runtime)
            .into_iter()
            .skip(1)
            .filter_map(|handle| runtime.actor(handle).map(|row| row.position)),
    ) {
        let ground = runtime.terrain().ground_height(position);
        if ground > maximum {
            maximum = ground;
            highest = position;
        }
    }
    maximum = maximum.wrapping_add(PROPERTIES.ground_clearance);
    if actor.position.z >= maximum {
        if actor.climb_hold != 0 {
            actor.climb_hold = actor.climb_hold.wrapping_sub(1);
        } else {
            actor.position.z = actor.position.z.wrapping_sub(PROPERTIES.descent);
        }
    } else {
        actor.position.z = actor.position.z.wrapping_add(
            if slope(highest, runtime.terrain()) > PROPERTIES.maximum_slope {
                PROPERTIES.steep_climb_step
            } else {
                PROPERTIES.climb_step
            },
        );
        actor.climb_hold = PROPERTIES.climb_hold;
    }
    runtime.write_actor(actor);
}

fn animate(handle: SourceEntityHandle, runtime: &mut dyn Runtime) {
    let Some(mut actor) = runtime.actor(handle) else {
        return;
    };
    if actor.length_or_ordinal >= 11 {
        let speed = match actor.animation_frame {
            96.. => 2u8,
            87..=95 => 3,
            60..=86 => 4,
            30..=59 => 5,
            _ => 6,
        };
        if actor.animation_frame != 0 && actor.animation_frame < 16 {
            runtime.sound(handle.source_slot, 48);
        }
        if actor.oscillation_flags & 1 != 0 {
            actor.animation_frame = actor.animation_frame.wrapping_add(speed);
            if actor.animation_frame > 100 {
                actor.animation_frame = 100;
                actor.oscillation_flags &= 0xfffe;
            }
        } else if actor.animation_frame > speed {
            actor.animation_frame -= speed;
        } else {
            actor.oscillation_flags = (actor.oscillation_flags | 1) ^ 2;
            actor.animation_frame = 0;
        }
    }
    actor.subspell_index = actor.subspell_index.wrapping_add(actor.rotation as u16) & 2047;
    if actor.phase & 3 == 0 {
        let magnitude = actor
            .rotation
            .wrapping_abs()
            .wrapping_sub(5)
            .max(PROPERTIES.minimum_rotation);
        actor.rotation = if actor.rotation <= 0 {
            magnitude.wrapping_neg()
        } else {
            magnitude
        };
    }
    runtime.write_actor(actor);
}

fn possess_root(handle: SourceEntityHandle, runtime: &mut dyn Runtime) {
    let Some(mut actor) = runtime.actor(handle) else {
        return;
    };
    if actor.cadence_counter != 0 {
        if actor.hit_source != 0 {
            actor.speed = actor
                .speed
                .wrapping_add((actor.damage >> 2) as i16)
                .max(actor.maximum_speed)
                .min(actor.minimum_speed);
            let source = runtime
                .target(actor.hit_source)
                .map_or(Position::default(), |target| target.position);
            actor.yaw = heading_for_positions(source, actor.position) as i16;
            actor.roll = actor.yaw;
            actor.hit_source = 0;
        }
        if actor.possessor != 0 {
            if actor.possessor != actor.owner {
                actor.owner = actor.possessor;
                actor.action = 177;
                actor.dword10 = 0;
                runtime.sound(actor.owner, 4);
            }
            actor.possessor = 0;
        }
    }
    if actor.life < 0 {
        actor.action = 181;
    }
    runtime.write_actor(actor);
}

fn propagate_colour(handle: SourceEntityHandle, runtime: &mut dyn Runtime) {
    let Some(root) = runtime.actor(handle) else {
        return;
    };
    let distance = (root.dword10 & 255) as i16;
    let centre = (root.dword10 >> 8) as i16;
    let base = colour_base(root.owner, runtime);
    let mut changed = false;
    for ordinal in [
        Some(centre.wrapping_add(distance)),
        (distance != 0).then(|| centre.wrapping_sub(distance)),
    ]
    .into_iter()
    .flatten()
    {
        if ordinal.wrapping_abs() > i16::from(root.length_or_ordinal) / 2 {
            continue;
        }
        let target = if ordinal == 0 {
            Some(handle)
        } else {
            chain(handle, runtime).into_iter().skip(1).find(|handle| {
                runtime
                    .actor(*handle)
                    .is_some_and(|row| i16::from(row.length_or_ordinal) == ordinal)
            })
        };
        if let Some(mut actor) = target.and_then(|handle| runtime.actor(handle))
            && let Some(visual) = colour_index(base, root.length_or_ordinal, ordinal as i8)
        {
            set_visual(&mut actor, visual, runtime);
            runtime.write_actor(actor);
            changed = true;
        }
    }
    if let Some(mut root) = runtime.actor(handle) {
        if changed {
            root.dword10 = i32::from(distance.wrapping_add(1)) | (i32::from(centre) << 8);
        } else {
            root.action = if root.owner != 0 { 178 } else { 176 };
        }
        runtime.write_actor(root);
    }
}

fn grow(handle: SourceEntityHandle, runtime: &mut dyn Runtime) {
    let Some(mut root) = runtime.actor(handle) else {
        return;
    };
    if root.target_or_growth != 0 {
        root.target_or_growth = root.target_or_growth.wrapping_sub(1);
        runtime.write_actor(root);
        return;
    }
    root.target_or_growth = PROPERTIES.growth_period;
    runtime.write_actor(root);
    if root.length_or_ordinal <= PROPERTIES.maximum_length - 2 {
        resize(handle, i16::from(root.length_or_ordinal) + 2, runtime);
    }
    if let Some(mut root) = runtime.actor(handle) {
        if root.mana < PROPERTIES.mana_growth_limit {
            root.mana = root.mana.wrapping_add(PROPERTIES.mana_growth);
        }
        runtime.write_actor(root);
    }
}

fn return_to_castle(handle: SourceEntityHandle, runtime: &mut dyn Runtime) {
    let Some(mut actor) = runtime.actor(handle) else {
        return;
    };
    if actor.phase & 31 != 0 {
        return;
    }
    // The source fast-motion branch precedes owner/player validation. A
    // still-accelerated creature retains even an invalid owner until slower.
    if actor.owner != 0 && actor.speed > actor.maximum_speed {
        return;
    }
    let player = runtime.player(actor.owner).filter(|player| {
        player.target.class == 3
            && player.target.life >= 0
            && player.target.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
            && player.castle_slot != 0
    });
    if actor.owner == 0 || player.is_none() {
        actor.dword10 = 0;
        actor.owner = 0;
        actor.action = 177;
    } else if actor.speed <= actor.maximum_speed {
        let player = player.expect("validated player");
        actor.roll = heading_for_positions(actor.position, player.castle_position) as i16;
        if actor.phase & 3 == 0
            && crate::legacy_sqrt(distance_squared_xy(actor.position, player.castle_position))
                <= 256
        {
            if actor.mana.wrapping_add(player.castle_mana) < player.castle_max_mana {
                actor.dword10 = PROPERTIES.shrink_delay;
                actor.action = 179;
            } else {
                actor.dword10 = 0;
                actor.owner = 0;
                actor.action = 177;
            }
        }
    }
    runtime.write_actor(actor);
}

fn shrink_into_castle(handle: SourceEntityHandle, runtime: &mut dyn Runtime) {
    let Some(mut root) = runtime.actor(handle) else {
        return;
    };
    if root.dword10 != 0 {
        root.dword10 = root.dword10.wrapping_sub(1);
        runtime.write_actor(root);
    } else if root.phase & 1 == 0 {
        if root.length_or_ordinal > 1 {
            resize(handle, i16::from(root.length_or_ordinal) - 2, runtime);
        } else {
            if let Some(player) = runtime.player(root.owner).filter(|player| {
                player.target.class == 3
                    && player.target.life >= 0
                    && player.target.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
                    && player.castle_slot != 0
            }) {
                runtime.add_castle_mana(player.castle_slot, root);
            }
            if let Some(mut root) = runtime.actor(handle) {
                root.hide();
                runtime.write_actor(root);
            }
        }
    }
}

fn distance(a: Position, b: Position) -> u32 {
    crate::legacy_sqrt(crate::native_world::level2_wrapping_source_distance_3d(
        a, b,
    ))
}

fn distance_squared_xy(a: Position, b: Position) -> u32 {
    let x = i32::from(a.x.wrapping_sub(b.x).cast_signed());
    let y = i32::from(a.y.wrapping_sub(b.y).cast_signed());
    x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y)) as u32
}

fn segment_offset(frame: u8, flags: u16, ordinal: i8, length: i8) -> i16 {
    let absolute = i32::from(ordinal).abs();
    let angle = ((15 - i32::from(length)) * absolute + absolute * i32::from(frame)) & 2047;
    let angle = if ordinal >= 0 {
        if flags & 2 != 0 { angle } else { 2048 - angle }
    } else if flags & 2 != 0 {
        1024 - angle
    } else {
        angle + 1024
    };
    (angle & 2047) as i16
}

fn position_segment(handle: SourceEntityHandle, runtime: &mut dyn Runtime) {
    let Some(mut actor) = runtime.actor(handle) else {
        return;
    };
    let Some(root) = actor.root.and_then(|handle| runtime.actor(handle)) else {
        // A recycled root address cannot adopt an old chain. Retire the orphan
        // through the same hide/next-pass reclaim path.
        actor.hide();
        runtime.write_actor(actor);
        return;
    };
    let Some(previous) = actor.previous.and_then(|handle| runtime.actor(handle)) else {
        actor.hide();
        runtime.write_actor(actor);
        return;
    };
    let previous = previous
        .previous
        .and_then(|handle| runtime.actor(handle))
        .unwrap_or(previous);
    actor.rotation = (root.subspell_index as i16).wrapping_add(segment_offset(
        root.animation_frame,
        root.oscillation_flags,
        actor.length_or_ordinal,
        root.length_or_ordinal,
    )) & 2047;
    actor.position = previous.position;
    crate::move_entity(
        &mut actor.position,
        actor.rotation,
        0,
        actor.half_width.wrapping_add(previous.half_width),
    );
    actor.position.z = previous
        .half_width
        .wrapping_sub(actor.half_width)
        .wrapping_add(previous.position.z);
    // 271D0 leaves the newly computed segment position in predictedAxis.
    runtime.terrain().set_source_predicted_position(actor.position);
    runtime.write_actor(actor);
}

fn interact_segment(handle: SourceEntityHandle, runtime: &mut dyn Runtime) {
    let Some(mut child) = runtime.actor(handle) else {
        return;
    };
    if child.cadence_counter == 0 {
        return;
    }
    let Some(mut root) = child.root.and_then(|handle| runtime.actor(handle)) else {
        return;
    };
    if !matches!(root.action, 176 | 178) {
        return;
    }
    if child.hit_source != 0 {
        root.speed = ((root.minimum_speed - root.maximum_speed) >> 2) + root.maximum_speed;
        root.yaw = heading_for_positions(
            runtime
                .target(child.hit_source)
                .map_or(Position::default(), |target| target.position),
            child.position,
        ) as i16;
        root.roll = root.yaw;
        let half = i32::from(root.length_or_ordinal >> 1);
        if half != 0 {
            let mut adjustment = 56 * i32::from(child.length_or_ordinal).abs() / half;
            if (root.yaw.wrapping_sub(child.rotation) & 2047) >= 1024 {
                adjustment = -adjustment;
            }
            let mut rotation = adjustment + i32::from(root.rotation);
            if rotation.abs() < i32::from(PROPERTIES.minimum_rotation) {
                rotation = if rotation <= 0 {
                    -i32::from(PROPERTIES.minimum_rotation)
                } else {
                    i32::from(PROPERTIES.minimum_rotation)
                };
            }
            rotation = rotation.clamp(
                -i32::from(PROPERTIES.maximum_rotation),
                i32::from(PROPERTIES.maximum_rotation),
            );
            root.rotation = rotation as i16;
        }
        for linked in chain(root.handle, runtime).into_iter().skip(1) {
            if let Some(mut actor) = runtime.actor(linked) {
                actor.hit_source = 0;
                runtime.write_actor(actor);
            }
        }
        child.hit_source = 0;
    }
    if child.possessor != 0 {
        if child.possessor != root.owner
            && (child.spell_amount != 0 || child.control_flags & 0x20_0000 == 0)
        {
            root.action = 177;
            root.dword10 = i32::from(child.length_or_ordinal) << 8;
            root.owner = child.possessor;
            runtime.sound(child.possessor, 4);
            if child.spell_amount != 0 {
                child.control_flags |= 0x20_0000;
            }
        }
        for linked in chain(root.handle, runtime).into_iter().skip(1) {
            if let Some(mut actor) = runtime.actor(linked) {
                actor.possessor = 0;
                runtime.write_actor(actor);
            }
        }
        child.possessor = 0;
    }
    runtime.write_actor(child);
    runtime.write_actor(root);
}
