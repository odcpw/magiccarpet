//! Wind controller state, linked kinematics and forced actor motion.

use super::State;
use crate::{FlightWorld, Position, heading_for_positions, move_entity};

/// sub_6D8B0: Wind contact awards belong only to an unsuppressed human
/// owner. The caller supplies the live source setting and fixed-address row.
pub(crate) fn award_experience(
    progression: &mut crate::native_world::Level2SharedSpellResetSession,
    owner: Option<&crate::native_world::EntitySnapshot>,
    source_settings: u8,
    count: u16,
) {
    let Some(owner) = owner.filter(|owner| {
        source_settings & 4 == 0
            && owner.index != 0
            && (owner.class, owner.model) == (3, 0)
            && progression
                .players()
                .iter()
                .any(|player| player.entity_index == owner.index)
    }) else {
        return;
    };
    if count != 0 {
        progression.award_spell_experience(owner.index, 21, i32::from(count));
    }
}

pub(crate) fn target_slots(
    map: &crate::renderable_map_chain::RenderableMapChains,
    position: Position,
) -> Vec<u16> {
    crate::level1_mission::source_map_search_slots(
        map,
        position,
        &crate::level1_mission::source_search_offsets(12),
    )
}

/// sub_33810 admission is shared by the actual actor and imported-row paths.
pub(crate) fn accepts(owner: u16, source_id: u16, class: u8, model: u8, action: u8) -> bool {
    match class {
        2 => matches!(model, 7 | 8),
        3 => source_id != owner && model != 2,
        5 => !matches!(action, 180 | 232) && !matches!(model, 10 | 15 | 18 | 27 | 28),
        10 => matches!(model, 13 | 14 | 39 | 57),
        _ => false,
    }
}

/// Every value comes from the authoritative actor at the source visit, rather
/// than a reset-time raw record which a typed creature may have superseded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Actor {
    pub(crate) position: Position,
    pub(crate) class: u8,
    pub(crate) model: u8,
    pub(crate) yaw: i16,
    pub(crate) move_heading: i16,
    pub(crate) rng: u16,
    pub(crate) flags: u32,
    pub(crate) ground_clearance: i16,
    pub(crate) descent: i16,
    pub(crate) half_height: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Motion {
    pub(crate) actor: Actor,
    pub(crate) actual_speed: Option<i16>,
    pub(crate) player_roll_delta: i16,
    pub(crate) contact: bool,
}

/// sub_33340's squared distance bands, wrapping RNG and forced movement.
/// The caller commits this to the same actor carrier used by its own callback
/// and submits contact through the existing combat route.
pub(crate) fn force_actor(
    parent: Wind,
    mut actor: Actor,
    terrain: &impl FlightWorld,
    cave: bool,
) -> Motion {
    let dx = i32::from(actor.position.x.wrapping_sub(parent.axis_position.x) as i16);
    let dy = i32::from(actor.position.y.wrapping_sub(parent.axis_position.y) as i16);
    let distance = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32;
    let airborne = actor.flags & 0x1000_0000 != 0;
    let player = (actor.class, actor.model) == (3, 0);
    let turn = if player { 56 } else { 204 };
    let band = if player { 384 } else { 768 };
    let mut speed = 0;
    let mut descent = 0;
    let mut contact = false;
    let mut actual_speed = None;
    let mut player_roll_delta = 0;
    if distance >= 3_211_264 {
        if airborne {
            actor.flags |= 0x800;
            actor.yaw = actor.yaw.wrapping_add(turn) & 2047;
            descent = actor.descent;
            if distance >= 5_308_416 {
                actor.flags &= !0x1000_0000;
            }
            speed = 64;
            contact = true;
        }
    } else if airborne {
        actor.flags |= 0x800;
        actor.yaw = actor.yaw.wrapping_add(turn) & 2047;
        actor.position.z = actor.position.z.wrapping_add(114);
        speed = 128;
        contact = true;
    } else {
        if player {
            player_roll_delta = 28;
            actual_speed = Some(80);
        }
        if distance >= 0x40000 {
            actor.yaw = (heading_for_positions(parent.axis_position, actor.position) as i16)
                .wrapping_add(591)
                & 2047;
            actor.move_heading = actor.yaw;
            speed = 96;
        } else {
            actor.flags |= 0x800;
            let relative_z = i32::from(actor.position.z) - i32::from(parent.word_0x30) + 57;
            actor.position = parent.axis_position;
            let ground = terrain.ground_height(actor.position);
            actor.position.z = (relative_z + i32::from(ground)) as i16;
            if actor.position.z < ground {
                actor.position.z = ground;
            }
            actor.yaw = actor.yaw.wrapping_add(turn) & 2047;
            actor.rng = actor.rng.wrapping_mul(9_377).wrapping_add(9_439);
            if relative_z >= i32::from(band + actor.rng % band) {
                actor.flags |= 0x1000_0000;
                actor.move_heading = actor.yaw;
            }
        }
    }
    move_entity(&mut actor.position, actor.move_heading, 0, speed);
    if cave && let Some(ceiling) = terrain.ceiling_height(actor.position) {
        actor.position.z = actor
            .position
            .z
            .min(ceiling.wrapping_sub(actor.half_height));
    }
    let ground = terrain.ground_height(actor.position);
    if actor.position.z > ground {
        actor.position.z = actor.position.z.wrapping_add(descent);
    }
    let floor = i32::from(ground) + i32::from(actor.ground_clearance);
    if i32::from(actor.position.z) < floor {
        actor.position.z = floor as i16;
    }
    Motion {
        actor,
        actual_speed,
        player_roll_delta,
        contact,
    }
}

pub(crate) const fn clear_flags(flags: u32) -> u32 {
    flags & 0xefff_f7ff
}

pub(crate) fn pulse_overlap(
    parent: Wind,
    position: Position,
    half_width: i16,
    half_depth: i16,
) -> bool {
    i32::from(position.x.wrapping_sub(parent.position.x) as i16).abs()
        < i32::from(parent.shift_rot_x) + i32::from(half_width)
        && i32::from(position.y.wrapping_sub(parent.position.y) as i16).abs()
            < i32::from(parent.shift_rot_x) + i32::from(half_depth)
}

impl Actor {
    pub(crate) fn from_source(
        source: &crate::native_world::EntitySnapshot,
        parameters: Option<&[u8]>,
    ) -> Self {
        let parameter_word = |offset| {
            parameters
                .and_then(|p| p.get(offset..offset + 2))
                .map_or(0, |b| i16::from_le_bytes([b[0], b[1]]))
        };
        Self {
            position: source.position,
            class: source.class,
            model: source.model,
            yaw: source.yaw,
            move_heading: source.source_spell_word_30() as i16,
            rng: source.entity_rng,
            flags: source.control_flags,
            ground_clearance: parameter_word(12),
            descent: parameter_word(14),
            half_height: source.array_fov(),
        }
    }
}

impl Motion {
    /// Imported source actors own this row directly. Typed families apply
    /// the same values to their family carrier before projecting the row.
    pub(crate) fn write_source(self, source: &mut crate::native_world::EntitySnapshot) {
        source.set_position(self.actor.position);
        source.set_yaw(self.actor.yaw);
        source.set_control_flags(self.actor.flags);
        source.set_entity_rng(self.actor.rng);
        source.record[52..54].copy_from_slice(&self.actor.move_heading.to_le_bytes());
        if let Some(speed) = self.actual_speed {
            source.set_actual_speed(speed);
        }
    }
}

/// Only allocator, map membership and per-slot RNG services are lent to the
/// field owner; no campaign/session behavior is reachable through this seam.
pub(crate) trait Arena {
    fn available(&self) -> usize;
    fn allocate(&mut self) -> Option<(u16, u16)>;
    fn release(&mut self, slot: u16, rng: u16);
    fn position(&mut self, slot: u16, position: Position) -> bool;
    fn rng(&mut self, slot: u16, rng: u16);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Definition {
    pub(crate) lifetime: i16,
    pub(crate) damage: u16,
    pub(crate) particle: [u16; 12],
    pub(crate) half_width: [i16; 12],
    pub(crate) half_depth: [i16; 12],
}

impl Default for Definition {
    fn default() -> Self {
        Self {
            lifetime: 500,
            damage: 1000,
            particle: [293, 294, 295, 296, 297, 298, 299, 300, 301, 302, 303, 304],
            half_width: [38, 44, 49, 55, 60, 66, 71, 82, 99, 121, 154, 198],
            half_depth: [31, 36, 40, 45, 49, 54, 58, 67, 81, 99, 126, 162],
        }
    }
}

impl Definition {
    pub(crate) fn with_content(
        mut self,
        content: crate::level_definition::FieldDefinition,
    ) -> Result<Self, String> {
        let crate::level_definition::FieldDefinition::Wind { lifetime, damage } = content else {
            return Err("wind constructor needs wind properties".to_owned());
        };
        self.lifetime = lifetime;
        self.damage = damage;
        Ok(self)
    }

    /// sub_4F1C0 binds the retained particle rows which also feed observation.
    pub(crate) fn from_particles(
        particles: &[crate::native_world::ParticleParameter],
    ) -> Result<Self, String> {
        let mut definition = Self::default();
        for (index, particle_id) in definition.particle.iter().copied().enumerate() {
            let particle = particles
                .get(usize::from(particle_id))
                .ok_or_else(|| format!("wind requires retained particle {particle_id}"))?;
            definition.half_width[index] = (550 * i32::from(particle.rotation_speed) / 1000) as i16;
            definition.half_depth[index] = (450 * i32::from(particle.rotation_speed) / 1000) as i16;
        }
        Ok(definition)
    }
}

impl State {
    pub(crate) fn construct_wind(
        &mut self,
        arena: &mut impl Arena,
        terrain: &impl FlightWorld,
        authored_index: u16,
        axis_position: Position,
        mut position: Position,
        definition: Definition,
    ) -> Option<Wind> {
        if arena.available() < 12 {
            return None;
        }
        let (slot, rng) = arena.allocate()?;
        let rng = rng.wrapping_mul(9_377).wrapping_add(9_439);
        position.z = terrain.ground_height(position);
        let yaw = (rng & 2047) as i16 - 1;
        let parent = Wind {
            source_entity_index: slot,
            entity_id: slot,
            entity_rng: rng,
            authored_index,
            source_class: 10,
            model: 22,
            action: 22,
            word_0x2c: 0,
            word_0x2e: 1,
            byte_0x38: 1,
            actual_speed: 50,
            minimum_speed: 20,
            maximum_speed: 10,
            max_life: definition.lifetime,
            life: definition.lifetime,
            subspell_index: definition.damage,
            control_flags: 0,
            word_0x30: 0,
            yaw,
            pitch: yaw,
            roll: yaw,
            particle_index: definition.particle[0],
            shift_rot_x: definition.half_width[0],
            shift_rot_y: definition.half_depth[0],
            axis_position,
            position,
            phase: slot as u8,
            active: true,
        };
        if !arena.position(slot, position) {
            arena.release(slot, rng);
            return None;
        }
        arena.rng(slot, rng);
        self.authored_model22.push(parent);
        let mut z_offset = 2_i16.wrapping_mul(definition.half_depth[0]);
        for segment_index in 1..=11_u8 {
            let Some((child_slot, _)) = arena.allocate() else {
                break;
            };
            let child_position = Position {
                z: position.z.wrapping_add(z_offset),
                ..position
            };
            let child = Segment {
                source_entity_index: child_slot,
                entity_id: slot,
                entity_rng: rng,
                max_life: definition.lifetime,
                life: definition.lifetime,
                parent_entity_index: slot,
                parent_entity_id: slot,
                source_class: 10,
                model: 75,
                action: 82,
                segment_index,
                word_0x2c: i16::from(segment_index),
                word_0x36: z_offset,
                yaw,
                pitch: yaw,
                roll: yaw,
                particle_index: definition.particle[usize::from(segment_index)],
                shift_rot_x: definition.half_width[usize::from(segment_index)],
                shift_rot_y: definition.half_depth[usize::from(segment_index)],
                position: child_position,
                phase: segment_index - 1,
                drawing_enabled: true,
                active: true,
            };
            if !arena.position(child_slot, child_position) {
                arena.release(child_slot, rng);
                break;
            }
            arena.rng(child_slot, rng);
            self.model22_spawned_children.push(child);
            z_offset = z_offset.wrapping_add(2_i16.wrapping_mul(child.shift_rot_y));
        }
        Some(parent)
    }

    /// Kinematics from sub_331A0. Phase is advanced by the source scheduler,
    /// which can therefore expose the original phase to both pulse consumers.
    pub(crate) fn step_wind(
        &mut self,
        arena: &mut impl Arena,
        terrain: &impl FlightWorld,
        slot: u16,
    ) -> bool {
        let Some(index) = self
            .authored_model22
            .iter()
            .position(|s| s.active && s.source_entity_index == slot)
        else {
            return false;
        };
        let mut parent = self.authored_model22[index];
        parent.life = parent.life.wrapping_sub(1);
        if parent.life < 0 {
            parent.control_flags |= 0x400;
            self.authored_model22[index] = parent;
            for child in &mut self.model22_spawned_children {
                if child.active && child.parent_entity_index == slot {
                    child.active = false;
                    child.drawing_enabled = false;
                }
            }
            return true;
        }
        parent.word_0x30 = parent.position.z;
        if parent.phase & 15 == 0 {
            parent.entity_rng = parent.entity_rng.wrapping_mul(9_377).wrapping_add(9_439);
            if parent.entity_rng & 1 == 0 {
                parent.word_0x2e = parent.word_0x2e.wrapping_neg();
            }
        }
        parent.roll = parent
            .roll
            .wrapping_add(11_i16.wrapping_mul(parent.word_0x2e))
            & 2047;
        let mut predicted = parent.position;
        move_entity(&mut predicted, parent.roll, 0, 32);
        parent.axis_position = predicted;
        parent.yaw = parent.yaw.wrapping_add(341) & 2047;
        move_entity(&mut predicted, parent.yaw, 0, 120);
        predicted.z = terrain.ground_height(predicted);
        parent.position = predicted;
        let mut previous = predicted;
        for child in self
            .model22_spawned_children
            .iter_mut()
            .filter(|s| s.active && s.parent_entity_index == slot)
        {
            child.yaw = heading_for_positions(child.position, previous) as i16;
            // Despite its name, EuclideanDistXYZ_58490 ignores Z and
            // returns the source integer square root of wrapped XY squares.
            let dx = i32::from(previous.x.wrapping_sub(child.position.x) as i16);
            let dy = i32::from(previous.y.wrapping_sub(child.position.y) as i16);
            let distance =
                crate::legacy_sqrt(dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32);
            let minimum_gap = 72 - 4 * (12 - child.word_0x2c);
            let mut position = child.position;
            if distance > minimum_gap as u32 {
                move_entity(
                    &mut position,
                    child.yaw,
                    0,
                    distance.wrapping_sub(minimum_gap as u32) as i16,
                );
            }
            position.z = parent.position.z.wrapping_add(child.word_0x36);
            child.position = position;
            previous = position;
            arena.position(child.source_entity_index, position);
        }
        arena.rng(slot, parent.entity_rng);
        self.authored_model22[index] = parent;
        arena.position(slot, parent.position)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Wind {
    pub source_entity_index: u16,
    pub entity_id: u16,
    pub entity_rng: u16,
    pub authored_index: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub word_0x2c: i16,
    pub word_0x2e: i16,
    pub byte_0x38: u8,
    pub actual_speed: i16,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub max_life: i16,
    pub life: i16,
    pub subspell_index: u16,
    pub control_flags: u32,
    pub word_0x30: i16,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub particle_index: u16,
    /// Source `array_0x52_82.pitch` from particle row 293, used as the
    /// parent's X half-width by `CompareAxisWithShift_10750`.
    pub shift_rot_x: i16,
    /// Source `array_0x52_82.fov`, the vertical half-height. The horizontal
    /// pitch/roll extents both use shift_rot_x in SetEntityShiftRot_49EA0.
    pub shift_rot_y: i16,
    pub axis_position: Position,
    pub position: Position,
    pub phase: u8,
    pub active: bool,
}

/// One class-10/model-75/action-82 row cloned by `AddWind_4F040`. `ApplyEvents`
/// excludes model 75 before consulting class 10's disabled action-82 slot; it
/// hides the row while the parent action-22 callback continues to own chain
/// movement and linked expiry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Segment {
    pub source_entity_index: u16,
    pub entity_id: u16,
    pub entity_rng: u16,
    pub max_life: i16,
    pub life: i16,
    pub parent_entity_index: u16,
    pub parent_entity_id: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub segment_index: u8,
    pub word_0x2c: i16,
    pub word_0x36: i16,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub particle_index: u16,
    pub shift_rot_x: i16,
    pub shift_rot_y: i16,
    pub position: Position,
    pub phase: u8,
    pub drawing_enabled: bool,
    pub active: bool,
}
