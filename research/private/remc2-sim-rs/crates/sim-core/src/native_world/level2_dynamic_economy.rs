use super::*;

impl crate::campaign::source_arena::SourceArena {
    /// Terminal NewEvent competes with raw and typed arena occupants through
    /// the same allocator as the ordinary source constructors.
    pub(super) fn step_possession_projectile_source(
        &mut self,
        terrain: &mut SessionTerrain,
        houses: &[HouseState],
        house_slots: &[u16],
        slot: u16,
    ) -> Level2PossessionStep {
        let mut step = self.economy.advance_possession_projectile_slot(
            slot,
            terrain,
            houses,
            house_slots,
            &self.entities,
            true,
        );
        if !step.pending_basic && !step.pending_splash && step.wizard_contact.is_none() {
            return step;
        }
        let Some(projectile) = self
            .economy
            .possession_projectiles
            .iter()
            .find(|row| row.source_entity_index == slot)
            .copied()
        else {
            return step;
        };
        let Some(child) = self.allocate_source_slot_with_terrain(terrain) else {
            step.wizard_contact = None;
            return step;
        };
        if step.pending_splash {
            if let Some(splash) = self.economy.initialize_authored_model5_water_splash(
                child,
                projectile.position,
                terrain,
            ) && let Some(row) = self
                .economy
                .authored_model5_water_splashes
                .iter_mut()
                .find(|row| row.source_entity_index == splash.source_entity_index)
            {
                row.entity_id = projectile.source_id_entity_index;
            }
        } else if step.pending_basic {
            if self
                .economy
                .initialize_possession_effect(child, projectile)
                .is_none()
            {
                return step;
            }
            let id = |slot: u16| {
                self.entities
                    .get(usize::from(slot))
                    .filter(|row| slot != 0 && row.active)
                    .map(|row| row.source_id)
                    .or_else(|| {
                        self.economy
                            .spheres
                            .iter()
                            .find(|row| slot != 0 && row.active && row.source_entity_index == slot)
                            .map(|row| row.source_entity_index)
                    })
            };
            step.accuracy = Some((
                projectile.source_id_entity_index,
                step.pending_contact
                    .and_then(id)
                    .zip(id(projectile.target_source_entity_index))
                    .is_some_and(|(hit, prior)| hit == prior),
            ));
            if step.pending_contact.is_some() {
                step.direct_hit_owner = Some(projectile.source_id_entity_index);
            }
        } else {
            self.economy
                .initialize_possession_attraction(child, projectile);
            let (_, target) = step.wizard_contact.expect("pending wizard constructor");
            let hit = self
                .entities
                .get(usize::from(target))
                .zip(
                    self.entities
                        .get(usize::from(projectile.target_source_entity_index)),
                )
                .is_some_and(|(target, previous)| target.source_id == previous.source_id);
            step.accuracy = Some((projectile.source_id_entity_index, hit));
        }
        if let Some(flyer) = self
            .economy
            .possession_projectiles
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
        {
            flyer.active = false;
        }
        if step.pending_basic || step.pending_splash {
            let position = if step.pending_splash {
                Position {
                    z: terrain.ground_height(projectile.position),
                    ..projectile.position
                }
            } else {
                projectile.position
            };
            crate::world::effects::sync_position(self, terrain, child, position, true, false)
                .expect("allocated Possession terminal effect has a valid fixed map slot");
        }
        step.live = false;
        step.pending_basic = false;
        step.pending_splash = false;
        step
    }
}

impl crate::world::fields::wind::Arena for Level2EconomyState {
    fn available(&self) -> usize {
        self.free_slots.len()
    }

    fn allocate(&mut self) -> Option<(u16, u16)> {
        let slot = self.allocate_slot()?;
        Some((slot, self.new_event_rng(slot)))
    }

    fn release(&mut self, slot: u16, rng: u16) {
        self.release_slot(slot, Some(rng));
    }

    fn position(&mut self, slot: u16, position: Position) -> bool {
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_ok()
    }

    fn rng(&mut self, slot: u16, rng: u16) {
        self.source_rng.insert(slot, rng);
    }
}

impl Level2EconomyState {
    /// `sub_6DCA0` applies `sub_4D380` after the common Fireball constructor.
    /// Its only differences are model28 and action29 for physical tier2.
    pub(super) fn apply_fireball_tier_constructor(&mut self, slot: u16, tier: i8) {
        if tier >= 2
            && let Some(projectile) = self
                .fireball_projectiles
                .iter_mut()
                .find(|projectile| projectile.active && projectile.source_entity_index == slot)
        {
            projectile.source_model = 28;
            projectile.action = 29;
        }
    }

    /// Source sub_11900's Castle contact mailbox. An already latched
    /// attacker causes replacement; an empty attacker lane accumulates.
    pub(crate) fn apply_source_castle_contact(
        &mut self,
        slot: u16,
        attacker_source_id: u16,
        damage: u16,
    ) -> bool {
        let Some(castle) = self.castle_at_mut(slot) else {
            return false;
        };
        castle.pending_damage = if castle.pending_attacker_entity_index != 0 {
            i32::from(damage)
        } else {
            castle.pending_damage.wrapping_add(i32::from(damage))
        };
        castle.pending_attacker_entity_index = attacker_source_id;
        true
    }

    /// sub_33710's Castle lane always adds the payload, independently of
    /// sub_11900's Contact replacement policy. The root's existing damage
    /// callback consumes these same pending words on its source visit.
    pub(crate) fn apply_wind_castle_contact(
        &mut self,
        slot: u16,
        _parent_slot: u16,
        owner: u16,
        payload: u16,
    ) -> bool {
        let Some(castle) = self.castle_at_mut(slot) else {
            return false;
        };
        castle.source_word_30 = 30;
        castle.pending_damage = castle.pending_damage.wrapping_add(i32::from(payload));
        castle.pending_attacker_entity_index = owner;
        true
    }

    /// Resolve a current typed actor before consulting the common source row.
    /// The supplied row owns descriptor/word30 fields which these carriers do
    /// not duplicate. Family-owned movement, RNG and flags override it here.
    pub(crate) fn wind_actor(
        &self,
        slot: u16,
        mut base: crate::world::fields::wind::Actor,
    ) -> Option<crate::world::fields::wind::Actor> {
        if let Some(actor) = self
            .model59_smoke_children
            .iter()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            base.position = actor.position;
            base.class = actor.source_class;
            base.model = actor.model;
            base.yaw = actor.yaw;
            base.rng = actor.entity_rng;
            base.flags = actor.control_flags | if actor.drawing_enabled { 0 } else { 0x400 };
            base.move_heading = actor.source_word_30 as i16;
            return Some(base);
        }
        if let Some(actor) = self
            .model13_spawned_children
            .iter()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            base.position = actor.position;
            base.class = actor.source_class;
            base.model = actor.model;
            base.yaw = actor.yaw;
            base.rng = actor.entity_rng;
            base.flags = actor.control_flags | if actor.drawing_enabled { 0 } else { 0x400 };
            base.move_heading = actor.source_word_30 as i16;
            return Some(base);
        }
        if let Some(actor) = self
            .bee_creatures
            .iter()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            base.position = actor.position;
            base.class = 5;
            base.model = actor.model;
            base.yaw = actor.yaw;
            base.rng = actor.entity_rng;
            base.flags = actor.control_flags;
            base.move_heading = actor.stage_duration;
            return Some(base);
        }
        if let Some(actor) = self
            .house_residents
            .iter()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            base.position = actor.position;
            base.class = 5;
            base.model = actor.model;
            base.yaw = actor.yaw;
            base.rng = actor.entity_rng;
            base.flags = actor.control_flags;
            base.move_heading = actor.stage.duration;
            return Some(base);
        }
        if let Some(actor) = self
            .authored_dragon_parts
            .iter()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            base.position = actor.position;
            base.class = 5;
            base.model = actor.model;
            base.yaw = actor.yaw;
            base.rng = actor.entity_rng;
            base.flags = actor.control_flags;
            return Some(base);
        }
        if let Some(actor) = self
            .authored_class5_model3_children
            .iter()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            base.position = actor.position;
            base.class = 5;
            base.model = actor.model;
            base.yaw = actor.yaw;
            base.rng = actor.entity_rng;
            base.flags = actor.control_flags;
            return Some(base);
        }
        if let Some(actor) = self
            .authored_class5
            .iter()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            base.position = actor.position;
            base.class = 5;
            base.model = actor.model;
            base.yaw = actor.yaw;
            base.rng = actor.entity_rng;
            if let Some(state) = self
                .authored_class5_archer
                .iter()
                .find(|s| s.source_entity_index == slot)
            {
                base.flags = state.control_flags;
                base.move_heading = state.stage.duration;
            } else if let Some(state) = self
                .authored_fireflies
                .iter()
                .find(|s| s.source_entity_index == slot)
            {
                base.flags = state.control_flags;
            } else if let Some(state) = self
                .authored_dragons
                .iter()
                .find(|s| s.source_entity_index == slot)
            {
                base.flags = state.control_flags;
            } else if let Some(state) = self
                .authored_class5_model3
                .iter()
                .find(|s| s.source_entity_index == slot)
            {
                base.flags = state.control_flags;
            }
            return Some(base);
        }
        if let Some(actor) = self
            .spheres
            .iter()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            base.position = actor.position;
            base.class = 10;
            base.model = 39;
            base.yaw = actor.yaw;
            base.rng = actor.entity_rng;
            base.flags = actor.control_flags;
            return Some(base);
        }
        None
    }

    /// Commit to the actual dynamic authority. The root publishes the common
    /// record/map projection in the same source visit after this succeeds.
    pub(crate) fn apply_wind_motion(
        &mut self,
        slot: u16,
        motion: crate::world::fields::wind::Motion,
    ) -> bool {
        if let Some(actor) = self
            .model59_smoke_children
            .iter_mut()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            actor.position = motion.actor.position;
            actor.yaw = motion.actor.yaw;
            actor.entity_rng = motion.actor.rng;
            actor.control_flags = motion.actor.flags;
            actor.source_word_30 = motion.actor.move_heading as u16;
        } else if let Some(actor) = self
            .model13_spawned_children
            .iter_mut()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            actor.position = motion.actor.position;
            actor.yaw = motion.actor.yaw;
            actor.entity_rng = motion.actor.rng;
            actor.control_flags = motion.actor.flags;
            actor.source_word_30 = motion.actor.move_heading as u16;
        } else if let Some(actor) = self
            .bee_creatures
            .iter_mut()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            actor.position = motion.actor.position;
            actor.yaw = motion.actor.yaw;
            actor.entity_rng = motion.actor.rng;
            actor.control_flags = motion.actor.flags;
            actor.stage_duration = motion.actor.move_heading;
            if let Some(speed) = motion.actual_speed {
                actor.actual_speed = speed;
            }
        } else if let Some(actor) = self
            .house_residents
            .iter_mut()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            actor.position = motion.actor.position;
            actor.yaw = motion.actor.yaw;
            actor.entity_rng = motion.actor.rng;
            actor.control_flags = motion.actor.flags;
            actor.stage.duration = motion.actor.move_heading;
            if let Some(speed) = motion.actual_speed {
                actor.actual_speed = speed;
            }
        } else if let Some(actor) = self
            .authored_dragon_parts
            .iter_mut()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            actor.position = motion.actor.position;
            actor.yaw = motion.actor.yaw;
            actor.entity_rng = motion.actor.rng;
            actor.control_flags = motion.actor.flags;
        } else if let Some(actor) = self
            .authored_class5_model3_children
            .iter_mut()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            actor.position = motion.actor.position;
            actor.yaw = motion.actor.yaw;
            actor.entity_rng = motion.actor.rng;
            actor.control_flags = motion.actor.flags;
            if let Some(speed) = motion.actual_speed {
                actor.actual_speed = speed;
            }
        } else if let Some(actor) = self
            .authored_class5
            .iter_mut()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            actor.position = motion.actor.position;
            actor.yaw = motion.actor.yaw;
            actor.entity_rng = motion.actor.rng;
            if let Some(speed) = motion.actual_speed {
                actor.actual_speed = speed;
            }
            if let Some(state) = self
                .authored_class5_archer
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
            {
                state.control_flags = motion.actor.flags;
                state.stage.duration = motion.actor.move_heading;
            } else if let Some(state) = self
                .authored_fireflies
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
            {
                state.control_flags = motion.actor.flags;
            } else if let Some(state) = self
                .authored_dragons
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
            {
                state.control_flags = motion.actor.flags;
            } else if let Some(state) = self
                .authored_class5_model3
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
            {
                state.control_flags = motion.actor.flags;
            }
        } else if let Some(actor) = self
            .spheres
            .iter_mut()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            actor.position = motion.actor.position;
            actor.yaw = motion.actor.yaw;
            actor.entity_rng = motion.actor.rng;
            actor.control_flags = motion.actor.flags;
        } else {
            return false;
        }
        self.source_rng.insert(slot, motion.actor.rng);
        true
    }

    pub(crate) fn clear_wind_flags(&mut self, slot: u16) -> bool {
        let clear = crate::world::fields::wind::clear_flags;
        if let Some(actor) = self
            .model59_smoke_children
            .iter_mut()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            actor.control_flags = clear(actor.control_flags);
        } else if let Some(actor) = self
            .model13_spawned_children
            .iter_mut()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            actor.control_flags = clear(actor.control_flags);
        } else if let Some(actor) = self.castle_at_mut(slot) {
            actor.control_flags = clear(actor.control_flags);
        } else if let Some(actor) = self
            .castle_defenders
            .iter_mut()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            actor.control_flags = clear(actor.control_flags);
        } else if let Some(actor) = self
            .house_residents
            .iter_mut()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            actor.control_flags = clear(actor.control_flags);
        } else if let Some(actor) = self
            .authored_dragon_parts
            .iter_mut()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            actor.control_flags = clear(actor.control_flags);
        } else if let Some(actor) = self
            .authored_class5_model3_children
            .iter_mut()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            actor.control_flags = clear(actor.control_flags);
        } else if let Some(actor) = self
            .bee_creatures
            .iter_mut()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            actor.control_flags = clear(actor.control_flags);
        } else if let Some(actor) = self
            .spheres
            .iter_mut()
            .find(|s| s.active && s.source_entity_index == slot)
        {
            actor.control_flags = clear(actor.control_flags);
        } else if self
            .authored_class5
            .iter()
            .any(|s| s.active && s.source_entity_index == slot)
        {
            if let Some(state) = self
                .authored_class5_archer
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
            {
                state.control_flags = clear(state.control_flags);
            } else if let Some(state) = self
                .authored_fireflies
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
            {
                state.control_flags = clear(state.control_flags);
            } else if let Some(state) = self
                .authored_dragons
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
            {
                state.control_flags = clear(state.control_flags);
            } else if let Some(state) = self
                .authored_class5_model3
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
            {
                state.control_flags = clear(state.control_flags);
            }
        } else {
            return false;
        }
        true
    }
}

/// Pinned `MSPRD0-0.TAB` half-dimensions used by model 41/action 43's
/// `sub_377F0` callback at VGA selector 1.  Model 42/action 44 does not use
/// this table: its source callback is 37BC0 and consumes DATA_BUILD00.
const LEVEL2_CASTLE_MODEL41_MSPRD_RECTS: [(u8, u8); 8] = [
    (0, 0),
    (16, 11),
    (16, 11),
    (16, 9),
    (16, 9),
    (16, 11),
    (1, 0),
    (1, 0),
];
const LEVEL2_CASTLE_CONSTRUCTION_INITIALIZED_BIT: u32 = 0x2;
/// `sub_65580` adds the model-39 source yaw (28 in the pinned Level 2
/// package) while a Possession flyer copies its collision contact.
pub(super) const POSSESSION_TARGET_Z_SHIFT: i16 = 28;
/// Source `sub_69640`'s tiered Possession constructor values. The direct
/// source profile supplies the physical tier; this arena helper only carries
/// the constructor identity and its `byte_0x44` selector into the live row.
const POSSESSION_TIERED_PROJECTILE_MODEL: u8 = 17;
const POSSESSION_TIERED_PROJECTILE_ACTION: u8 = 18;
const FIREFLY_FIREBALL_PITCH_TURN_LIMIT: i16 = 22;

const fn possession_source_byte_0x44(physical_tier: u8) -> u8 {
    match physical_tier {
        0 => 12,
        1 => 54,
        _ => 69,
    }
}
/// `sub_112D0` scans the source effect's complete five-by-five map-cell
/// square when model 12 has pitch 512: Y is the outer source loop and X the
/// inner loop.
const POSSESSION_IMPACT_SEARCH_OFFSETS: [(i8, i8); 25] = [
    (-2, -2),
    (-1, -2),
    (0, -2),
    (1, -2),
    (2, -2),
    (-2, -1),
    (-1, -1),
    (0, -1),
    (1, -1),
    (2, -1),
    (-2, 0),
    (-1, 0),
    (0, 0),
    (1, 0),
    (2, 0),
    (-2, 1),
    (-1, 1),
    (0, 1),
    (1, 1),
    (2, 1),
    (-2, 2),
    (-1, 2),
    (0, 2),
    (1, 2),
    (2, 2),
];

fn level2_castle_model41_msprd_rect(stage: u8) -> Option<(u8, u8)> {
    (1..=7)
        .contains(&stage)
        .then(|| LEVEL2_CASTLE_MODEL41_MSPRD_RECTS[usize::from(stage)])
}

fn level2_castle_construction_height_from_square(
    planes: &TerrainPlanes,
    x: u8,
    y: u8,
    width: u8,
    height: u8,
) -> i16 {
    let right = x.wrapping_add(width);
    let bottom = y.wrapping_add(height);
    (i16::from(planes.ground_heights[terrain_index(x, y)])
        + i16::from(planes.ground_heights[terrain_index(right, y)])
        + i16::from(planes.ground_heights[terrain_index(right, bottom)])
        + i16::from(planes.ground_heights[terrain_index(x, bottom)]))
        >> 2
}

fn level2_castle_projectile_distance(from: Position, to: Position) -> i32 {
    let dx = i32::from(i16::from_ne_bytes(from.x.wrapping_sub(to.x).to_ne_bytes()).unsigned_abs());
    let dy = i32::from(i16::from_ne_bytes(from.y.wrapping_sub(to.y).to_ne_bytes()).unsigned_abs());
    let dz = i32::from(from.z.wrapping_sub(to.z).unsigned_abs());
    dx.max(dy).max(dz)
}

fn level2_move_castle_projectile_toward(projectile: &mut Level2CastleProjectile, target: Position) {
    if projectile.actual_speed < projectile.minimum_speed {
        projectile.actual_speed = projectile.actual_speed.saturating_add(2);
    } else if projectile.actual_speed > projectile.minimum_speed {
        projectile.actual_speed = projectile.actual_speed.saturating_sub(2);
    }
    let dx = i16::from_ne_bytes(target.x.wrapping_sub(projectile.position.x).to_ne_bytes());
    let dy = i16::from_ne_bytes(target.y.wrapping_sub(projectile.position.y).to_ne_bytes());
    let dz = target.z.wrapping_sub(projectile.position.z);
    let yaw = if dx.unsigned_abs() >= dy.unsigned_abs() {
        if dx >= 0 { 512 } else { 1_536 }
    } else if dy >= 0 {
        1_024
    } else {
        0
    };
    let pitch = if dz < 0 {
        256
    } else if dz > 0 {
        -256
    } else {
        0
    };
    projectile.yaw = yaw;
    projectile.pitch = pitch;
    crate::move_entity(
        &mut projectile.position,
        yaw,
        pitch,
        projectile.actual_speed,
    );
}

fn level2_castle_nearby_projectile(castle: Level2CastleState, position: Position) -> bool {
    let dx = i32::from(
        i16::from_ne_bytes(castle.position.x.wrapping_sub(position.x).to_ne_bytes()).unsigned_abs(),
    );
    let dy = i32::from(
        i16::from_ne_bytes(castle.position.y.wrapping_sub(position.y).to_ne_bytes()).unsigned_abs(),
    );
    dx <= i32::from(castle.shift_pitch).saturating_add(2_560)
        && dy <= i32::from(castle.shift_roll).saturating_add(2_560)
}

fn level2_entity_nearby_projectile(entity: &EntitySnapshot, position: Position) -> bool {
    let dx = i32::from(
        i16::from_ne_bytes(entity.position.x.wrapping_sub(position.x).to_ne_bytes()).unsigned_abs(),
    );
    let dy = i32::from(
        i16::from_ne_bytes(entity.position.y.wrapping_sub(position.y).to_ne_bytes()).unsigned_abs(),
    );
    dx <= i32::from(entity.array_pitch()) + 2_560 && dy <= i32::from(entity.array_roll()) + 2_560
}

/// Frozen source-list handles; pointed-to state is resolved live per callback.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Level2CastleAuxiliaryMembership {
    replacement_list_epoch: u32,
    class3_source_slots: Vec<crate::campaign::source_arena::SourceEntityHandle>,
    model15_source_slots: Vec<crate::campaign::source_arena::SourceEntityHandle>,
    class5_source_slots: Vec<crate::campaign::source_arena::SourceEntityHandle>,
}

/// The source-shaped payload handed to `sub_69AB0`'s post-allocation
/// `sub_68E50(parent, flyer, holder)` carrier. The parent/player route
/// computes the hand displacement, terrain rollback, FOV Z offset, and
/// wrapping pitch before calling the economy allocator; keeping those values
/// together prevents the allocator from silently falling back to the raw
/// player position and zero pitch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2CastleProjectileLaunchPayload {
    pub position: Position,
    pub axis_position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub source_id_entity_index: u16,
    pub source_subspell_index: u16,
    pub source_mana: i32,
    pub source_a4_byte_154: u8,
}

/// Source fields copied by `sub_693F0` after `sub_6DCA0` returns the ordinary
/// class-9/model-0 Fireball row.  The producer computes the hand/auxiliary
/// transform; the economy owns only the arena product and its callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2FireballProjectileLaunchPayload {
    pub origin: FireballProjectileOrigin,
    pub position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub axis_position: Position,
    pub source_id_entity_index: u16,
    pub holder_entity_index: u16,
    pub source_subspell_index: u16,
    pub source_mana: i32,
    pub source_dword_0x10: i32,
    pub target_source_entity_index: u16,
    pub target_class: u8,
    pub target_model: u8,
}

/// One live source-list row eligible for Fireball target acquisition. The
/// caller supplies rows in remc2 list order: class 3 first, then class-5
/// model buckets 0..28 excluding model 22.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Level2FireballGuidanceTarget {
    pub source_slot: u16,
    pub source_id: u16,
    pub class: u8,
    pub model: u8,
    pub top_position: Position,
}

fn fireball_target_selector_matches(
    projectile: &Level2FireballProjectile,
    class: u8,
    model: u8,
) -> bool {
    !matches!(
        projectile.origin,
        FireballProjectileOrigin::Wyvern
            | FireballProjectileOrigin::Creature18
            | FireballProjectileOrigin::Creature21
            | FireballProjectileOrigin::Cymmerian
            | FireballProjectileOrigin::FinalBoss
            | FireballProjectileOrigin::Creature27
    ) || projectile.target_class == u8::MAX
        || (projectile.target_class == class
            && (projectile.target_model == u8::MAX || projectile.target_model == model))
}

/// One map-linked collision row read by `sub_10780` after flyer movement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Level2FireballCollisionTarget {
    pub source_slot: u16,
    pub source_id: u16,
    pub class: u8,
    pub model: u8,
    pub position: Position,
    pub half_width: i16,
    pub vertical_offset: i16,
    pub half_height: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Level2FireballStep {
    pub live: bool,
    pub impact_slot: Option<u16>,
    pub reflection: Option<super::support_spells::ReboundReflection>,
    pub pending_birth: Option<FireballBirth>,
    pub direct_hit_owner: Option<u16>,
    pub direct_hit_family: Option<u8>,
    pub collision_target: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FireballBirth {
    Impact,
    Ash,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Level2PossessionStep {
    pub live: bool,
    pending_basic: bool,
    pending_splash: bool,
    pending_contact: Option<u16>,
    pub direct_hit_owner: Option<u16>,
    pub accuracy: Option<(u16, bool)>,
    pub wizard_contact: Option<(u16, u16)>,
}

impl Level2PossessionStep {
    const fn live(live: bool) -> Self {
        Self {
            live,
            pending_basic: false,
            pending_splash: false,
            pending_contact: None,
            direct_hit_owner: None,
            accuracy: None,
            wizard_contact: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Level2Class10Model0Step {
    pub live: bool,
    pub first_impact: bool,
    pub damage_pulse: bool,
    pub owner_entity_index: u16,
    pub payload: u16,
    pub position: Position,
}

impl Level2EconomyState {
    #[must_use]
    pub fn source_owner(&self, source_entity_index: u16) -> u16 {
        self.source_owner
            .get(&source_entity_index)
            .copied()
            .unwrap_or_default()
    }

    /// Source `sub_69900`: allocate the ordinary tier-0 class-9/model-1
    /// flyer. The caller deliberately performs the holder's expenditure and
    /// decrement independently, because source does both even when this
    /// allocation cannot obtain an arena row.
    pub fn spawn_possession_projectile(
        &mut self,
        launch_position: Position,
        source_id_entity_index: u16,
        holder_entity_index: u16,
        yaw: i16,
        pitch: i16,
        player_speed: i16,
        axis_position: Position,
    ) -> Option<u16> {
        self.spawn_possession_projectile_with_physical_tier(
            launch_position,
            source_id_entity_index,
            holder_entity_index,
            yaw,
            pitch,
            player_speed,
            axis_position,
            0,
        )
    }

    /// Source `sub_69640`'s constructor split: physical tier zero follows
    /// `sub_69900`, while tiers one and two allocate class-9/model-17,
    /// action-18 rows and carry the pinned byte-0x44 selector (54/69).
    /// Allocation remains transactional with the existing map-link and RNG
    /// setup, so a failed row cannot publish an audio receipt upstream.
    pub fn spawn_possession_projectile_with_physical_tier(
        &mut self,
        launch_position: Position,
        source_id_entity_index: u16,
        holder_entity_index: u16,
        yaw: i16,
        pitch: i16,
        player_speed: i16,
        axis_position: Position,
        physical_tier: u8,
    ) -> Option<u16> {
        let physical_tier = physical_tier.min(2);
        let tiered = physical_tier != 0;
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let projectile = Level2PossessionProjectile {
            source_entity_index,
            entity_rng,
            source_id_entity_index,
            holder_entity_index,
            source_class: POSSESSION_PROJECTILE_CLASS,
            source_model: if tiered {
                POSSESSION_TIERED_PROJECTILE_MODEL
            } else {
                POSSESSION_PROJECTILE_MODEL
            },
            action: if tiered {
                POSSESSION_TIERED_PROJECTILE_ACTION
            } else {
                POSSESSION_PROJECTILE_ACTION
            },
            output_class: POSSESSION_IMPACT_CLASS,
            // The source's byte-0x44 selector is the constructor's second
            // subtype argument. The existing Level2 row exposes that field
            // as `output_model`; tier zero therefore retains the old model-12
            // value while tiered rows retain 54/69 for downstream dispatch.
            output_model: possession_source_byte_0x44(physical_tier),
            target_source_entity_index: 0,
            acquisition_attempted: false,
            threat_accounted: false,
            position: launch_position,
            axis_position,
            yaw: yaw & 0x07ff,
            pitch: pitch & 0x07ff,
            roll: 0,
            fov: 0,
            actual_speed: POSSESSION_PROJECTILE_SPEED.wrapping_add(player_speed),
            minimum_speed: POSSESSION_PROJECTILE_SPEED,
            life: POSSESSION_PROJECTILE_LIFETIME_TICKS,
            max_life: POSSESSION_PROJECTILE_LIFETIME_TICKS,
            phase: 0,
            active: true,
        };
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: launch_position.x,
                y: launch_position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, Some(entity_rng));
            return None;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        self.possession_projectiles.push(projectile);
        Some(source_entity_index)
    }

    /// Source `sub_6DCA0(..., a3=0)` / `sub_693F0` product for the ordinary
    /// public family-0 cast. Allocation, NewEvent RNG, map linkage, and the
    /// source payload are committed together; a failed map link rolls back the
    /// slot without publishing a partial row.
    pub(super) fn spawn_fireball_projectile_with_payload(
        &mut self,
        payload: Level2FireballProjectileLaunchPayload,
        player_speed: i16,
    ) -> Option<u16> {
        let source_entity_index = self.allocate_slot()?;
        self.initialize_fireball_projectile(source_entity_index, payload, player_speed)
    }

    pub(super) fn initialize_fireball_projectile(
        &mut self,
        source_entity_index: u16,
        payload: Level2FireballProjectileLaunchPayload,
        player_speed: i16,
    ) -> Option<u16> {
        let entity_rng = self.new_event_rng(source_entity_index);
        let projectile = Level2FireballProjectile {
            origin: payload.origin,
            source_entity_index,
            entity_rng,
            owner_entity_index: payload.source_id_entity_index,
            holder_entity_index: payload.holder_entity_index,
            source_subspell_index: payload.source_subspell_index,
            source_mana: payload.source_mana,
            source_class: FIREBALL_PROJECTILE_CLASS,
            source_model: FIREBALL_PROJECTILE_MODEL,
            action: 0,
            payload: i32::from(payload.source_subspell_index),
            position: payload.position,
            axis_position: payload.axis_position,
            yaw: payload.yaw & 0x07ff,
            pitch: payload.pitch & 0x07ff,
            roll: payload.yaw & 0x07ff,
            fov: payload.pitch & 0x07ff,
            actual_speed: FIREBALL_SPEED
                .wrapping_add(player_speed)
                .clamp(FIREBALL_SPEED, 0x2000),
            minimum_speed: FIREBALL_SPEED,
            life: FIREBALL_LIFETIME_TICKS,
            max_life: FIREBALL_LIFETIME_TICKS,
            dword_0x10: payload.source_dword_0x10,
            target_source_entity_index: payload.target_source_entity_index,
            target_class: payload.target_class,
            target_model: payload.target_model,
            acquisition_attempted: !matches!(
                payload.origin,
                FireballProjectileOrigin::PlayerSpell
                    | FireballProjectileOrigin::Wyvern
                    | FireballProjectileOrigin::Creature18
                    | FireballProjectileOrigin::Creature21
                    | FireballProjectileOrigin::Cymmerian
                    | FireballProjectileOrigin::FinalBoss
                    | FireballProjectileOrigin::Creature27
            ),
            threat_accounted: false,
            control_flags: 0,
            source_word_2c: 0,
            phase: source_entity_index as u8,
            active: true,
            drawing_disabled: false,
        };
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: payload.position.x,
                y: payload.position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, Some(entity_rng));
            return None;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        self.source_owner
            .insert(source_entity_index, payload.source_id_entity_index);
        self.fireball_projectiles.push(projectile);
        Some(source_entity_index)
    }

    pub(crate) fn initialize_eruption_projectile(
        &mut self,
        slot: u16,
        position: Position,
        yaw: i16,
        destination: Position,
        owner: u16,
        holder: u16,
    ) -> Option<u16> {
        let slot = self.initialize_fireball_projectile(
            slot,
            Level2FireballProjectileLaunchPayload {
                origin: FireballProjectileOrigin::Eruption,
                position,
                yaw,
                pitch: -386,
                axis_position: destination,
                source_id_entity_index: owner,
                holder_entity_index: holder,
                source_subspell_index: 100,
                source_mana: 50,
                source_dword_0x10: 0,
                target_source_entity_index: 0,
                target_class: 0,
                target_model: 0,
            },
            0,
        )?;
        self.fireball_projectiles
            .iter_mut()
            .find(|row| row.source_entity_index == slot)?
            .life = 1;
        Some(slot)
    }

    /// Source `sub_1CC20` applies `sub_5EF70` to the bound player only after
    /// the creature's NewEvent allocation succeeds. Keep that target-side
    /// entity write beside the shared projectile constructor so callers
    /// cannot publish feedback for a failed allocation.
    pub(super) fn apply_creature_fireball_target_feedback(
        imported_entities: &mut [EntitySnapshot],
        target_entity_index: u16,
    ) -> bool {
        let Some(target) = imported_entities
            .get_mut(usize::from(target_entity_index))
            .filter(|target| {
                target.active && target.life >= 0 && target.class == 3 && target.model == 0
            })
        else {
            return false;
        };
        target.apply_source_sub_5ef70();
        true
    }

    /// Submit one source-shaped Fireball damage mailbox write. The target is
    /// resolved by class/model and live map relation at write time, and
    /// ordinary area writer accumulates two distinct hits before the Bee
    /// callback. The impact's own callback latch prevents duplicate pulses;
    /// the victim mailbox must not discard a second producer's hit.
    pub(super) fn submit_authored_bee_damage(
        &mut self,
        target_slot: u16,
        damage: i32,
        attacker: u16,
    ) -> bool {
        if damage <= 0 || attacker == 0 || !self.map_chains.is_linked(target_slot) {
            return false;
        }
        let Some(bee) = self.bee_creatures.iter_mut().find(|bee| {
            bee.active
                && bee.drawing_enabled
                && bee.life >= 0
                && bee.source_entity_index == target_slot
                && bee.entity_id != attacker
                && bee.source_class == 5
                && bee.model == 2
        }) else {
            return false;
        };
        crate::combat::actors::DamageMailboxMut::new(
            &mut bee.pending_damage,
            &mut bee.pending_attacker_entity_index,
        )
        .submit(
            damage,
            attacker,
            crate::combat::actors::DamageWriter::Ordinary,
        );
        true
    }

    /// Source `CastPlayerFire_65B30 -> sub_65C20` callback for the live
    /// class-9/model-0 flyer. Guidance and collision happen here; RNG, damage,
    /// sound, and the short lifetime belong to the distinct class-10 impact.
    /// A failed impact allocation leaves the flyer live at the terminal
    /// position so the next scheduled callback retries the constructor.
    pub(super) fn step_fireball_projectile_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
        imported_entities: &[EntitySnapshot],
    ) -> Level2FireballStep {
        self.advance_fireball_projectile_slot(source_slot, terrain, imported_entities, false)
    }

    pub(super) fn advance_fireball_projectile_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
        imported_entities: &[EntitySnapshot],
        defer_birth: bool,
    ) -> Level2FireballStep {
        self.advance_fireball_projectile_with_views(
            source_slot,
            terrain,
            imported_entities,
            defer_birth,
            None,
        )
        .expect("legacy Fireball advance does not use current-address views")
    }

    pub(super) fn advance_fireball_projectile_with_views(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
        imported_entities: &[EntitySnapshot],
        defer_birth: bool,
        views: Option<&[std::result::Result<Option<super::source_target::View>, String>]>,
    ) -> std::result::Result<Level2FireballStep, String> {
        let guidance_targets = self.fireball_guidance_targets(imported_entities);
        let collision_targets = if views.is_none() {
            self.fireball_collision_targets(imported_entities)
        } else {
            Vec::new()
        };
        let Some(projectile_index) = self.fireball_projectiles.iter().position(|projectile| {
            projectile.active && projectile.source_entity_index == source_slot
        }) else {
            return Ok(Level2FireballStep {
                live: false,
                impact_slot: None,
                reflection: None,
                pending_birth: None,
                direct_hit_owner: None,
                direct_hit_family: None,
                collision_target: None,
            });
        };
        let mut projectile = self.fireball_projectiles[projectile_index];

        if matches!(
            projectile.origin,
            FireballProjectileOrigin::Firefly
                | FireballProjectileOrigin::Dragon
                | FireballProjectileOrigin::Worm
        ) {
            if let Some(target) =
                imported_entities.get(usize::from(projectile.target_source_entity_index))
                && target.active
                && target.life >= 0
                && target.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
            {
                let (_, desired_pitch) =
                    projectile_guidance_angles(projectile.position, target.position);
                projectile.roll = projectile.yaw;
                projectile.fov = desired_pitch;
                projectile.pitch = projectile.pitch.wrapping_add(turn_toward(
                    projectile.pitch,
                    desired_pitch,
                    FIREFLY_FIREBALL_PITCH_TURN_LIMIT,
                )) & 0x07ff;
            }
        } else if matches!(
            projectile.origin,
            FireballProjectileOrigin::Wyvern
                | FireballProjectileOrigin::Creature18
                | FireballProjectileOrigin::Creature21
                | FireballProjectileOrigin::Cymmerian
                | FireballProjectileOrigin::FinalBoss
                | FireballProjectileOrigin::Creature27
        ) && projectile.target_source_entity_index != 0
        {
            // Source24510/1D460 bind descriptor61 (113/113); source1CC20 binds
            // descriptor65 (0/22). sub65610 follows the bound fixed address.
            let (yaw_turn, pitch_turn) = match projectile.origin {
                FireballProjectileOrigin::Wyvern | FireballProjectileOrigin::Creature18 => {
                    (113, 113)
                }
                FireballProjectileOrigin::FinalBoss => (17, 22),
                FireballProjectileOrigin::Creature27 => (
                    crate::world::effects::MODEL27_GUIDANCE.yaw_turn,
                    crate::world::effects::MODEL27_GUIDANCE.pitch_turn,
                ),
                _ => (0, 22),
            };
            let target_position = if let Some(views) = views {
                let target = views
                    .get(usize::from(projectile.target_source_entity_index))
                    .ok_or("Fireball target address is outside arena")?
                    .as_ref()
                    .map_err(Clone::clone)?;
                match target {
                    Some(target) if target.model == 2 => Some(target.position),
                    Some(target) => {
                        let shape = target.collision.ok_or_else(|| {
                            format!(
                                "Fireball target {} lacks current source axes",
                                projectile.target_source_entity_index
                            )
                        })?;
                        Some(super::guided_projectile::centre(
                            target.position,
                            target.model,
                            shape.vertical_offset,
                        ))
                    }
                    None => None,
                }
            } else {
                guidance_targets
                    .iter()
                    .find(|target| target.source_slot == projectile.target_source_entity_index)
                    .map(|target| target.top_position)
                    .or_else(|| {
                        imported_entities
                            .get(usize::from(projectile.target_source_entity_index))
                            .map(|target| {
                                super::guided_projectile::centre(
                                    target.position,
                                    target.model,
                                    target.array_yaw(),
                                )
                            })
                    })
            };
            if let Some(target_position) = target_position {
                (projectile.roll, projectile.fov) =
                    projectile_guidance_angles(projectile.position, target_position);
                projectile.yaw = projectile.yaw.wrapping_add(turn_toward(
                    projectile.yaw,
                    projectile.roll,
                    yaw_turn,
                )) & 2047;
                projectile.pitch = projectile.pitch.wrapping_add(turn_toward(
                    projectile.pitch,
                    projectile.fov,
                    pitch_turn,
                )) & 2047;
            }
        } else if projectile.target_source_entity_index == 0 {
            if !projectile.acquisition_attempted {
                projectile.acquisition_attempted = true;
                let mut best = None;
                let mut best_score = u32::MAX;
                for target in guidance_targets
                    .iter()
                    .copied()
                    .filter(|target| target.source_id != projectile.owner_entity_index)
                    .filter(|target| {
                        fireball_target_selector_matches(&projectile, target.class, target.model)
                    })
                {
                    let Some(score) = projectile_target_score(
                        projectile.position,
                        projectile.yaw,
                        projectile.pitch,
                        target.top_position,
                    ) else {
                        continue;
                    };
                    if score < best_score {
                        best = Some(target);
                        best_score = score;
                    }
                }
                if let Some(target) = best {
                    projectile.target_source_entity_index = target.source_slot;
                    (projectile.roll, projectile.fov) =
                        projectile_guidance_angles(projectile.position, target.top_position);
                    projectile.yaw = projectile.yaw.wrapping_add(turn_toward(
                        projectile.yaw,
                        projectile.roll,
                        FIREBALL_INITIAL_YAW_TURN_LIMIT,
                    )) & 0x07ff;
                    projectile.pitch = projectile.fov;
                } else {
                    projectile.roll = projectile.yaw;
                    projectile.fov = projectile.pitch;
                }
            }
        } else if let Some(target) = guidance_targets
            .iter()
            .find(|target| target.source_slot == projectile.target_source_entity_index)
        {
            (projectile.roll, projectile.fov) =
                projectile_guidance_angles(projectile.position, target.top_position);
            projectile.yaw = projectile.yaw.wrapping_add(turn_toward(
                projectile.yaw,
                projectile.roll,
                FIREBALL_HOMING_YAW_TURN_LIMIT,
            )) & 0x07ff;
            projectile.pitch = projectile.pitch.wrapping_add(turn_toward(
                projectile.pitch,
                projectile.fov,
                FIREBALL_HOMING_PITCH_TURN_LIMIT,
            )) & 0x07ff;
        }

        // sub_65820 adjusts the source speed by two toward minSpeed before
        // moving, including player-launched shots that inherit extra speed.
        let speed_delta = i32::from(projectile.minimum_speed) - i32::from(projectile.actual_speed);
        projectile.actual_speed = projectile
            .actual_speed
            .wrapping_add((2 * speed_delta.signum()) as i16);
        crate::move_entity(
            &mut projectile.position,
            projectile.yaw,
            projectile.pitch,
            projectile.actual_speed,
        );

        let offsets = if views.is_some() {
            super::guided_projectile::collision_search_offsets(FIREBALL_COLLISION_HALF_WIDTH)
        } else {
            source_search_offsets(1)
        };
        let candidates = source_map_search_slots(
            &self.map_chains,
            projectile.position,
            &offsets,
        );
        let mut current_hit_position = None;
        let collision_slot = if let Some(views) = views {
            let mut hit = None;
            for candidate_slot in candidates {
                let Some(target) = views
                    .get(usize::from(candidate_slot))
                    .ok_or("Fireball map candidate is outside arena")?
                    .as_ref()
                    .map_err(Clone::clone)?
                else {
                    continue;
                };
                if target.control_flags & 8 == 0
                    || target.source_id == projectile.owner_entity_index
                    || !fireball_target_selector_matches(&projectile, target.class, target.model)
                {
                    continue;
                }
                let shape = target.collision.ok_or_else(|| {
                    format!("Fireball collision target {candidate_slot} lacks current source axes")
                })?;
                let dx =
                    i32::from(target.position.x.wrapping_sub(projectile.position.x) as i16).abs();
                let dy =
                    i32::from(target.position.y.wrapping_sub(projectile.position.y) as i16).abs();
                let dz = (i32::from(target.position.z) + i32::from(shape.vertical_offset)
                    - i32::from(projectile.position.z)
                    - i32::from(FIREBALL_COLLISION_VERTICAL_OFFSET))
                .abs();
                if dx < i32::from(FIREBALL_COLLISION_HALF_WIDTH) + i32::from(shape.half_width)
                    && dy < i32::from(FIREBALL_COLLISION_HALF_WIDTH) + i32::from(shape.half_depth)
                    && dz < i32::from(FIREBALL_COLLISION_HALF_HEIGHT) + i32::from(shape.half_height)
                {
                    // sub_65820 temporarily raises the victim by its source
                    // array.yaw (except model2) before copying its position.
                    current_hit_position = Some(super::guided_projectile::centre(
                        target.position,
                        target.model,
                        shape.vertical_offset,
                    ));
                    hit = Some(candidate_slot);
                    break;
                }
            }
            hit
        } else {
            candidates.into_iter().find(|candidate_slot| {
                collision_targets.iter().any(|candidate| {
                    candidate.source_slot == *candidate_slot
                        && candidate.source_id != projectile.owner_entity_index
                        && fireball_target_selector_matches(
                            &projectile,
                            candidate.class,
                            candidate.model,
                        )
                        && crate::level1_mission::source_entities_overlap(
                            projectile.position,
                            FIREBALL_COLLISION_HALF_WIDTH,
                            FIREBALL_COLLISION_VERTICAL_OFFSET,
                            FIREBALL_COLLISION_HALF_HEIGHT,
                            candidate.position,
                            candidate.half_width,
                            candidate.vertical_offset,
                            candidate.half_height,
                        )
                })
            })
        };

        let mut terminal = collision_slot.is_some();
        if let Some(target) = collision_slot
            .and_then(|slot| imported_entities.get(usize::from(slot)))
            .filter(|target| target.active && target.control_flags & 0x8010 != 0
                && (views.is_none() || self.live_arena_cursor_row(target.index).is_none()))
        {
            use crate::spells::support::{
                ReboundProjectile, ReboundSpread, ReboundTarget, reflect_projectile,
            };
            // All products of this flyer callback deliver class10/model0,
            // one of sub_68740's explicitly reflectable source products.
            let mut reflected = ReboundProjectile {
                owner: projectile.owner_entity_index,
                target: projectile.target_source_entity_index,
                position: projectile.position,
                yaw: projectile.yaw,
                pitch: projectile.pitch,
                roll: projectile.roll,
                fov: projectile.fov,
                damage: projectile.source_subspell_index,
                mana: projectile.source_mana,
                life: i32::from(projectile.life),
                max_life: i32::from(projectile.max_life),
                rng: projectile.entity_rng,
            };
            if let Some(mana_after) = reflect_projectile(
                &mut reflected,
                ReboundTarget {
                    source_id: target.source_id,
                    flags: target.control_flags,
                    mana: target.mana,
                    position: target.position,
                    fov: target.array_fov(),
                },
                ReboundSpread::Narrow,
            ) {
                projectile.owner_entity_index = reflected.owner;
                projectile.target_source_entity_index = reflected.target;
                projectile.position = reflected.position;
                projectile.yaw = reflected.yaw;
                projectile.pitch = reflected.pitch;
                projectile.roll = reflected.roll;
                projectile.fov = reflected.fov;
                projectile.source_subspell_index = reflected.damage;
                projectile.payload = i32::from(reflected.damage);
                projectile.life = reflected.life as i16;
                projectile.entity_rng = reflected.rng;
                self.source_owner.insert(source_slot, reflected.owner);
                self.source_rng.insert(source_slot, reflected.rng);
                self.fireball_projectiles[projectile_index] = projectile;
                if views.is_none() {
                    self.map_chains
                        .sync_entity(RenderableMapSeed {
                            entity_index: source_slot,
                            active: true,
                            x: projectile.position.x,
                            y: projectile.position.y,
                        })
                        .expect("live reflected Fireball retains its arena map slot");
                }
                return Ok(Level2FireballStep {
                    live: true,
                    impact_slot: None,
                    pending_birth: None,
                    direct_hit_owner: None,
                    direct_hit_family: None,
                    collision_target: None,
                    reflection: Some(super::support_spells::ReboundReflection {
                        target_slot: target.index,
                        mana_after,
                    }),
                });
            }
        }
        if let Some(position) = current_hit_position {
            projectile.position = position;
        } else if let Some(target_slot) = collision_slot
            && let Some(target) = collision_targets
                .iter()
                .find(|target| target.source_slot == target_slot)
        {
            projectile.position = target.position;
        } else {
            let ground = terrain.ground_height(projectile.position);
            let contact_height = if ground > projectile.position.z {
                Some(ground)
            } else {
                terrain
                    .ceiling_height(projectile.position)
                    .map(|ceiling| ceiling.wrapping_sub(FIREBALL_COLLISION_HALF_HEIGHT))
                    .filter(|limit| projectile.position.z > *limit)
            };
            if let Some(height) = contact_height {
                // sub_65820 retains moved XY and replaces only Z. The floor
                // branch short-circuits before the cave ceiling branch.
                projectile.position.z = height;
                if terrain.material_mask(projectile.position) == 1 {
                    if defer_birth {
                        self.fireball_projectiles[projectile_index] = projectile;
                        return Ok(Level2FireballStep {
                            live: true,
                            impact_slot: None,
                            reflection: None,
                            pending_birth: Some(FireballBirth::Ash),
                            direct_hit_owner: None,
                            direct_hit_family: None,
                            collision_target: None,
                        });
                    }
                    if let Some(splash) =
                        self.allocate_authored_model5_water_splash(projectile.position, terrain)
                    {
                        if let Some(row) = self
                            .authored_model5_water_splashes
                            .iter_mut()
                            .find(|row| row.source_entity_index == splash.source_entity_index)
                        {
                            row.entity_id = projectile.owner_entity_index;
                        }
                        self.source_owner
                            .insert(splash.source_entity_index, projectile.owner_entity_index);
                    }
                    projectile.active = false;
                    projectile.drawing_disabled = true;
                    self.fireball_projectiles[projectile_index] = projectile;
                    return Ok(Level2FireballStep {
                        live: false,
                        impact_slot: None,
                        reflection: None,
                        pending_birth: None,
                        direct_hit_owner: None,
                        direct_hit_family: None,
                        collision_target: None,
                    });
                }
                terminal = true;
            } else {
                projectile.life = projectile.life.wrapping_sub(1);
                terminal = projectile.life < 0;
            }
        }

        self.fireball_projectiles[projectile_index] = projectile;
        if views.is_none() {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: projectile.position.x,
                y: projectile.position.y,
            });
        }
        if !terminal {
            return Ok(Level2FireballStep {
                live: true,
                impact_slot: None,
                reflection: None,
                pending_birth: None,
                direct_hit_owner: None,
                direct_hit_family: None,
                collision_target: None,
            });
        }

        if defer_birth {
            return Ok(Level2FireballStep {
                live: true,
                impact_slot: None,
                reflection: None,
                pending_birth: Some(FireballBirth::Impact),
                direct_hit_owner: collision_slot.map(|_| projectile.owner_entity_index),
                direct_hit_family: None,
                collision_target: collision_slot,
            });
        }
        let impact_slot = if projectile.origin == FireballProjectileOrigin::Eruption {
            self.allocate_authored_model17(u16::MAX, projectile.position, terrain)
                .map(|birth| {
                    let slot = birth.source_entity_index;
                    if let Some(source) = self
                        .authored_model17
                        .iter_mut()
                        .find(|row| row.source_entity_index == slot)
                    {
                        source.entity_id = projectile.owner_entity_index;
                    }
                    self.source_owner
                        .insert(slot, projectile.owner_entity_index);
                    slot
                })
        } else {
            self.allocate_class10_model0_effect(
                projectile.source_entity_index,
                projectile.owner_entity_index,
                projectile.yaw,
                projectile.pitch,
                projectile.position,
                projectile.source_subspell_index,
                0,
                0x0002_0000,
                128,
                128,
            )
        };
        if impact_slot.is_some() {
            let projectile = &mut self.fireball_projectiles[projectile_index];
            projectile.active = false;
            projectile.drawing_disabled = true;
        }
        Ok(Level2FireballStep {
            live: impact_slot.is_none(),
            impact_slot,
            reflection: None,
            pending_birth: None,
            direct_hit_owner: impact_slot
                .and(collision_slot)
                .map(|_| projectile.owner_entity_index),
            direct_hit_family: None,
            collision_target: impact_slot.and(collision_slot),
        })
    }

    /// Rebuild the source target-list view consumed by `sub_67CB0`: live
    /// class-3 rows first, followed by class-5 model buckets in numeric model
    /// order.  The imported rows retain their package link order; typed Bee
    /// rows join their model-2 bucket by fixed source slot.
    pub(super) fn fireball_guidance_targets(
        &self,
        imported_entities: &[EntitySnapshot],
    ) -> Vec<Level2FireballGuidanceTarget> {
        let class3_slots = imported_entities
            .iter()
            .filter(|entity| {
                entity.index != 0
                    && entity.active
                    && entity.life >= 0
                    && entity.class == 3
                    && entity.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
            })
            .map(|entity| entity.index)
            .collect::<BTreeSet<_>>();
        let linked = class3_slots
            .iter()
            .filter_map(|slot| imported_entities.get(usize::from(*slot))?.next_relation)
            .collect::<BTreeSet<_>>();
        let mut targets = Vec::new();
        if let Some(mut current) = class3_slots
            .iter()
            .copied()
            .find(|slot| !linked.contains(slot))
        {
            let mut visited = BTreeSet::new();
            while current != 0 && visited.insert(current) {
                let Some(entity) = imported_entities.get(usize::from(current)) else {
                    break;
                };
                if class3_slots.contains(&current) {
                    let collision = entity.collision_target();
                    targets.push(Level2FireballGuidanceTarget {
                        source_slot: current,
                        source_id: entity.source_id,
                        class: entity.class,
                        model: entity.model,
                        top_position: Position {
                            z: entity
                                .position
                                .z
                                .wrapping_add(collision.collision_vertical_offset),
                            ..entity.position
                        },
                    });
                }
                current = entity.next_relation.unwrap_or(0);
            }
        }

        for model in 0..=28 {
            if model == 22 {
                continue;
            }
            let mut bucket = imported_entities
                .iter()
                .filter(|entity| {
                    entity.index != 0
                        && entity.active
                        && entity.life >= 0
                        && entity.class == 5
                        && entity.model == model
                        && entity.record_bytes().get(61).copied().unwrap_or_default() != 0
                        && entity.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
                })
                .map(|entity| {
                    let collision = entity.collision_target();
                    Level2FireballGuidanceTarget {
                        source_slot: entity.index,
                        source_id: entity.source_id,
                        class: entity.class,
                        model: entity.model,
                        top_position: Position {
                            z: entity
                                .position
                                .z
                                .wrapping_add(collision.collision_vertical_offset),
                            ..entity.position
                        },
                    }
                })
                .collect::<Vec<_>>();
            if model == 2 {
                bucket.extend(self.bee_creatures.iter().filter_map(|bee| {
                    (bee.active && bee.drawing_enabled && bee.life >= 0 && bee.source_byte_39 != 0)
                        .then_some(Level2FireballGuidanceTarget {
                            source_slot: bee.source_entity_index,
                            source_id: bee.entity_id,
                            class: 5,
                            model: 2,
                            top_position: bee.position,
                        })
                }));
            }
            if matches!(
                model,
                level2_class5::archer::ARCHER_MODEL | level2_class5::firefly::FIREFLY_MODEL
            ) {
                bucket.extend(self.authored_class5.iter().filter_map(|source| {
                    (source.active
                        && source.drawing_enabled
                        && source.life >= 0
                        && source.model == model
                        && source.source_byte_39 != 0)
                        .then_some(Level2FireballGuidanceTarget {
                            source_slot: source.source_entity_index,
                            source_id: source.source_entity_index,
                            class: 5,
                            model: source.model,
                            top_position: source.position,
                        })
                }));
            }
            if model == level2_class5::dragon::DRAGON_MODEL {
                bucket.extend(self.authored_class5.iter().filter_map(|source| {
                    let render = self
                        .authored_dragons
                        .iter()
                        .find(|render| render.source_entity_index == source.source_entity_index)?;
                    (source.active
                        && source.drawing_enabled
                        && source.life >= 0
                        && source.model == model
                        && render.control_flags & 8 != 0
                        && render.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
                        && self.map_chains.is_linked(source.source_entity_index))
                    .then_some(Level2FireballGuidanceTarget {
                        source_slot: source.source_entity_index,
                        source_id: source.source_entity_index,
                        class: 5,
                        model: source.model,
                        top_position: Position {
                            z: source.position.z.wrapping_add(render.render_yaw),
                            ..source.position
                        },
                    })
                }));
                bucket.extend(self.authored_dragon_parts.iter().filter_map(|part| {
                    (part.active
                        && part.drawing_enabled
                        && part.life >= 0
                        && part.model == model
                        && self.map_chains.is_linked(part.source_entity_index))
                    .then_some(Level2FireballGuidanceTarget {
                        source_slot: part.source_entity_index,
                        source_id: part.entity_id,
                        class: 5,
                        model: part.model,
                        top_position: Position {
                            z: part.position.z.wrapping_add(part.render_yaw),
                            ..part.position
                        },
                    })
                }));
            }
            bucket.sort_unstable_by_key(|target| target.source_slot);
            targets.extend(bucket);
        }
        targets
    }

    /// Project the map-linked rows needed by `sub_10780`. The qualified
    /// Level-2 route exercises imported class-3 rows, typed Bee rows, and the
    /// typed Castle root; all retain the source low-byte collision flag and
    /// fixed slot identity used by the map chain.
    pub(super) fn fireball_collision_targets(
        &self,
        imported_entities: &[EntitySnapshot],
    ) -> Vec<Level2FireballCollisionTarget> {
        let mut targets = imported_entities
            .iter()
            .filter(|entity| {
                entity.index != 0
                    && entity.active
                    && entity.life >= 0
                    && entity.control_flags & 8 != 0
                    && entity.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
                    && self.map_chains.is_linked(entity.index)
            })
            .map(|entity| {
                let collision = entity.collision_target();
                Level2FireballCollisionTarget {
                    source_slot: entity.index,
                    source_id: entity.source_id,
                    class: entity.class,
                    model: entity.model,
                    position: entity.position,
                    half_width: collision.collision_half_width,
                    vertical_offset: collision.collision_vertical_offset,
                    half_height: collision.collision_half_height,
                }
            })
            .collect::<Vec<_>>();
        targets.extend(self.bee_creatures.iter().filter_map(|bee| {
            (bee.active
                && bee.drawing_enabled
                && bee.life >= 0
                && bee.control_flags & 8 != 0
                && self.map_chains.is_linked(bee.source_entity_index))
            .then_some(Level2FireballCollisionTarget {
                source_slot: bee.source_entity_index,
                source_id: bee.entity_id,
                class: 5,
                model: 2,
                position: bee.position,
                half_width: LEVEL2_BEE_COLLISION_HALF_EXTENT,
                vertical_offset: 0,
                half_height: LEVEL2_BEE_COLLISION_HALF_EXTENT,
            })
        }));
        targets.extend(self.castle_defenders.iter().filter_map(|actor| {
            (actor.active
                && actor.drawing_enabled()
                && actor.life >= 0
                && actor.control_flags & 8 != 0
                && self.map_chains.is_linked(actor.source_entity_index))
            .then_some(Level2FireballCollisionTarget {
                source_slot: actor.source_entity_index,
                source_id: actor.id_entity_index,
                class: 5,
                model: 15,
                position: actor.position,
                half_width: actor.shift_rot_x,
                vertical_offset: actor.fov,
                half_height: actor.shift_rot_y,
            })
        }));
        targets.extend(self.authored_class5.iter().filter_map(|source| {
            (source.active
                && source.drawing_enabled
                && source.life >= 0
                && matches!(
                    source.model,
                    crate::goat::GOAT_MODEL
                        | level2_class5::archer::ARCHER_MODEL
                        | level2_class5::firefly::FIREFLY_MODEL
                )
                && self.map_chains.is_linked(source.source_entity_index))
            .then_some(Level2FireballCollisionTarget {
                source_slot: source.source_entity_index,
                source_id: if source.model == level2_class5::archer::ARCHER_MODEL {
                    self.authored_class5_archer
                        .iter()
                        .find(|actor| actor.source_entity_index == source.source_entity_index)
                        .map_or(source.source_entity_index, |actor| actor.entity_id)
                } else {
                    source.source_entity_index
                },
                class: 5,
                model: source.model,
                position: source.position,
                half_width: source.shift_rot_x,
                vertical_offset: if source.model == level2_class5::archer::ARCHER_MODEL {
                    level2_class5::archer::ARCHER_COLLISION_VERTICAL_OFFSET
                } else if source.model == crate::goat::GOAT_MODEL {
                    crate::goat::GOAT_CONSTRUCTOR_COLLISION_HALF_EXTENT
                } else {
                    0
                },
                half_height: source.shift_rot_y,
            })
        }));
        targets.extend(self.authored_class5.iter().filter_map(|source| {
            let render = self
                .authored_dragons
                .iter()
                .find(|render| render.source_entity_index == source.source_entity_index)?;
            (source.active
                && source.drawing_enabled
                && source.life >= 0
                && source.model == level2_class5::dragon::DRAGON_MODEL
                && render.control_flags & 8 != 0
                && render.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
                && self.map_chains.is_linked(source.source_entity_index))
            .then_some(Level2FireballCollisionTarget {
                source_slot: source.source_entity_index,
                source_id: source.source_entity_index,
                class: 5,
                model: source.model,
                position: source.position,
                half_width: render.render_pitch,
                vertical_offset: render.render_yaw,
                half_height: render.render_fov,
            })
        }));
        targets.extend(self.authored_dragon_parts.iter().filter_map(|part| {
            (part.active
                && part.drawing_enabled
                && part.life >= 0
                && self.map_chains.is_linked(part.source_entity_index))
            .then_some(Level2FireballCollisionTarget {
                source_slot: part.source_entity_index,
                source_id: part.entity_id,
                class: 5,
                model: part.model,
                position: part.position,
                half_width: part.render_pitch,
                vertical_offset: part.render_yaw,
                half_height: part.render_fov,
            })
        }));
        for castle in self.live_castles().filter(|castle| {
            castle.life >= 0
                && castle.control_flags & 8 != 0
                && castle.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
                && self.map_chains.is_linked(castle.source_entity_index)
        }) {
            targets.push(Level2FireballCollisionTarget {
                source_slot: castle.source_entity_index,
                source_id: castle.source_id_entity_index,
                class: castle.source_class,
                model: castle.source_model,
                position: castle.position,
                half_width: castle.shift_pitch,
                vertical_offset: castle.shift_yaw,
                half_height: castle.shift_fov,
            });
        }
        targets
    }

    /// Source `NewAdd0A0C_4E8C0` product for a successful possession flyer
    /// collision. This is intentionally private to the projectile callback:
    /// public production routes cannot create an ownership pulse directly.
    fn allocate_possession_effect(
        &mut self,
        projectile: Level2PossessionProjectile,
    ) -> Option<u16> {
        let source_entity_index = self.allocate_slot()?;
        self.initialize_possession_effect(source_entity_index, projectile)
    }

    fn initialize_possession_effect(
        &mut self,
        source_entity_index: u16,
        projectile: Level2PossessionProjectile,
    ) -> Option<u16> {
        let entity_rng = self.new_event_rng(source_entity_index);
        let effect = Level2PossessionEffect {
            source_entity_index,
            entity_rng,
            source_id_entity_index: projectile.source_id_entity_index,
            projectile_entity_index: projectile.source_entity_index,
            source_class: projectile.output_class,
            source_model: projectile.output_model,
            action: POSSESSION_IMPACT_ACTION,
            position: projectile.position,
            yaw: projectile.yaw,
            pitch: projectile.pitch,
            life: POSSESSION_IMPACT_LIFE,
            max_life: POSSESSION_IMPACT_LIFE,
            phase: 0,
            active: true,
        };
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: effect.position.x,
                y: effect.position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, Some(entity_rng));
            return None;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        self.possession_effects.push(effect);
        Some(source_entity_index)
    }

    /// Source `CastPosses_65F60`: acquire/refresh model-1 guidance before the
    /// first move, strictly clamp the flyer before its source-ordered map scan,
    /// and retire only after model-12 allocation succeeds. A target contact
    /// shifts to that target's temporary source-top Z before allocation;
    /// allocation failure preserves that shifted flyer and its unchanged life
    /// for the next callback. On a miss, only a boundary still violated after
    /// the scan or signed lifetime expiry attempts model 12.
    pub(super) fn step_possession_projectile_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
        houses: &[HouseState],
        house_guidance_slots: &[u16],
        entities: &[EntitySnapshot],
    ) -> Level2PossessionStep {
        self.advance_possession_projectile_slot(
            source_slot,
            terrain,
            houses,
            house_guidance_slots,
            entities,
            false,
        )
    }

    fn advance_possession_projectile_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
        houses: &[HouseState],
        house_guidance_slots: &[u16],
        entities: &[EntitySnapshot],
        defer_birth: bool,
    ) -> Level2PossessionStep {
        let Some(projectile_index) = self.possession_projectiles.iter().position(|projectile| {
            projectile.active && projectile.source_entity_index == source_slot
        }) else {
            return Level2PossessionStep::live(false);
        };
        let mut moved = self.possession_projectiles[projectile_index];

        // `CastPosses_65F60` resolves word_0x96 on entry. A sentinel target
        // permits exactly one `sub_67CB0` attempt; success copies roll/fov
        // directly into yaw/pitch before movement. A pre-existing relation
        // instead runs `sub_65610`, refreshing desired angles and capping both
        // turns by descriptor-2's 0x71 source limit.
        if moved.target_source_entity_index == 0 {
            if !moved.acquisition_attempted {
                moved.acquisition_attempted = true;
                if let Some((target_source_entity_index, target_top)) =
                    level2_possession_guidance_target(
                        &self.spheres,
                        &self.possession_guidance_source_slots,
                        houses,
                        house_guidance_slots,
                        entities,
                        moved,
                    )
                {
                    moved.target_source_entity_index = target_source_entity_index;
                    (moved.roll, moved.fov) =
                        projectile_guidance_angles(moved.position, target_top);
                    moved.yaw = moved.roll;
                    moved.pitch = moved.fov;
                }
            }
        } else if let Some(target_top) = level2_possession_guidance_relation(
            &self.spheres,
            houses,
            &self.possession_projectiles,
            &self.possession_guidance_stale_records,
            moved.target_source_entity_index,
        ) {
            (moved.roll, moved.fov) = projectile_guidance_angles(moved.position, target_top);
            moved.yaw = moved.yaw.wrapping_add(turn_toward(
                moved.yaw,
                moved.roll,
                PROJECTILE_ACQUISITION_ANGLE,
            )) & 0x07ff;
            moved.pitch = moved.pitch.wrapping_add(turn_toward(
                moved.pitch,
                moved.fov,
                PROJECTILE_ACQUISITION_ANGLE,
            )) & 0x07ff;
        }
        if moved.source_model == POSSESSION_TIERED_PROJECTILE_MODEL {
            // Source `sub_662E0` (model 17/action 18) eases actual speed by
            // two units toward minSpeed before movement. Basic model 1 keeps
            // the producer's inherited player-speed value unchanged.
            let speed_delta = moved.minimum_speed.wrapping_sub(moved.actual_speed);
            if speed_delta != 0 {
                moved.actual_speed =
                    moved
                        .actual_speed
                        .wrapping_add(if speed_delta <= 0 { -2 } else { 2 });
            }
        }
        crate::move_entity(
            &mut moved.position,
            moved.yaw,
            moved.pitch,
            moved.actual_speed,
        );
        moved.phase = moved.phase.wrapping_add(1);

        if moved.source_model == POSSESSION_TIERED_PROJECTILE_MODEL {
            return self.finish_tiered_possession_move(projectile_index, moved, terrain, entities);
        }

        // `CastPosses_65F60` strictly clamps the flyer before its `sub_108B0`
        // scan. These constraints only choose the scan position: an ordinary
        // initial clamp is not a model-12 impact and equality is not contact.
        let ground_height = terrain.ground_height(moved.position);
        if ground_height > moved.position.z {
            moved.position.z = ground_height;
        }
        let cave_limit = terrain
            .ceiling_height(moved.position)
            .map(|ceiling_height| {
                ceiling_height.wrapping_sub(POSSESSION_PROJECTILE_COLLISION_HALF_HEIGHT)
            });
        if let Some(cave_limit) = cave_limit {
            if cave_limit < moved.position.z {
                moved.position.z = cave_limit;
            }
        }

        let target = level2_first_possession_target(&self.map_chains, &self.spheres, houses, moved);
        if let Some((target_slot, target_top)) = target {
            moved.position = target_top;
            self.possession_projectiles[projectile_index] = moved;
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: moved.position.x,
                y: moved.position.y,
            });
            if defer_birth {
                return Level2PossessionStep {
                    pending_basic: true,
                    pending_contact: Some(target_slot),
                    ..Level2PossessionStep::live(true)
                };
            }
            if self.allocate_possession_effect(moved).is_some() {
                let projectile = &mut self.possession_projectiles[projectile_index];
                projectile.active = false;
                return Level2PossessionStep::live(false);
            }
            return Level2PossessionStep::live(true);
        }

        // A source-ordered clamp normally makes both strict comparisons false.
        // Recheck them after the miss because a representable ground/ceiling
        // conflict can leave the second clamp outside the first bound. Only
        // that still-violated state, or signed lifetime expiry, reaches model 12.
        let terminal = if ground_height > moved.position.z
            || cave_limit.is_some_and(|limit| limit < moved.position.z)
        {
            true
        } else {
            moved.life = moved.life.wrapping_sub(1);
            moved.life < 0
        };
        self.possession_projectiles[projectile_index] = moved;
        let _ = self.map_chains.sync_entity(RenderableMapSeed {
            entity_index: source_slot,
            active: true,
            x: moved.position.x,
            y: moved.position.y,
        });
        if terminal && defer_birth {
            return Level2PossessionStep {
                pending_basic: true,
                ..Level2PossessionStep::live(true)
            };
        }
        if terminal && self.allocate_possession_effect(moved).is_some() {
            self.possession_projectiles[projectile_index].active = false;
            return Level2PossessionStep::live(false);
        }
        Level2PossessionStep::live(true)
    }

    /// sub_662E0. Its 54/69 selectors both call AddAuxiliary_50500
    /// (Events.cpp constructor table), never the model12 possession pulse.
    fn finish_tiered_possession_move(
        &mut self,
        projectile_index: usize,
        mut moved: Level2PossessionProjectile,
        terrain: &SessionTerrain,
        entities: &[EntitySnapshot],
    ) -> Level2PossessionStep {
        const HALF_HEIGHT: i16 = 2 * POSSESSION_PROJECTILE_COLLISION_VERTICAL_OFFSET;
        let mut targets = self.fireball_collision_targets(entities);
        targets.extend(
            self.spheres
                .iter()
                .filter(|sphere| {
                    sphere.active
                        && !sphere.pending_delete
                        && sphere.control_flags & 8 != 0
                        && self.map_chains.is_linked(sphere.source_entity_index)
                })
                .map(|sphere| Level2FireballCollisionTarget {
                    source_slot: sphere.source_entity_index,
                    source_id: sphere.source_entity_index,
                    class: 10,
                    model: 39,
                    position: sphere.position,
                    half_width: sphere.collision_pitch,
                    vertical_offset: sphere.collision_yaw,
                    half_height: sphere.collision_fov,
                }),
        );
        let contact =
            source_map_search_slots(&self.map_chains, moved.position, &source_search_offsets(1))
                .into_iter()
                .find_map(|slot| {
                    targets
                        .iter()
                        .find(|target| {
                            target.source_slot == slot
                                && target.source_id != moved.source_id_entity_index
                                && crate::level1_mission::source_entities_overlap(
                                    moved.position,
                                    POSSESSION_PROJECTILE_COLLISION_HALF_WIDTH,
                                    POSSESSION_PROJECTILE_COLLISION_VERTICAL_OFFSET,
                                    HALF_HEIGHT,
                                    target.position,
                                    target.half_width,
                                    target.vertical_offset,
                                    target.half_height,
                                )
                        })
                        .copied()
                });
        let mut terminal = contact.is_some();
        if let Some(contact) = contact {
            moved.position = contact.position;
            if entities
                .get(usize::from(contact.source_slot))
                .is_none_or(|row| row.model != 2)
            {
                moved.position.z = moved.position.z.wrapping_add(contact.vertical_offset);
            }
        } else {
            let ground = terrain.ground_height(moved.position);
            let ceiling = terrain
                .ceiling_height(moved.position)
                .map(|z| z.wrapping_sub(HALF_HEIGHT));
            let boundary = if ground > moved.position.z {
                Some(ground)
            } else {
                ceiling.filter(|z| moved.position.z > *z)
            };
            if let Some(boundary) = boundary {
                moved.position.z = boundary;
                if terrain.material_mask(moved.position) == 1 {
                    moved.active = false;
                    self.possession_projectiles[projectile_index] = moved;
                    return Level2PossessionStep {
                        pending_splash: true,
                        ..Level2PossessionStep::live(false)
                    };
                }
                terminal = true;
            } else {
                moved.life = moved.life.wrapping_sub(1);
                terminal = moved.life < 0;
            }
        }
        let wizard = contact
            .and_then(|contact| entities.get(usize::from(contact.source_slot)))
            .filter(|row| row.active && row.class == 3 && matches!(row.model, 0 | 1));
        let mut step = Level2PossessionStep::live(true);
        if terminal {
            if let Some(wizard) = wizard {
                step.wizard_contact = Some((moved.source_id_entity_index, wizard.index));
            } else {
                moved.active = false;
            }
            if !moved.active {
                let previous_target = entities.get(usize::from(moved.target_source_entity_index));
                step.accuracy = Some((
                    moved.source_id_entity_index,
                    contact
                        .zip(previous_target)
                        .is_some_and(|(contact, target)| contact.source_id == target.source_id),
                ));
            }
        }
        step.live = moved.active;
        self.possession_projectiles[projectile_index] = moved;
        let _ = self.map_chains.sync_entity(RenderableMapSeed {
            entity_index: moved.source_entity_index,
            active: true,
            x: moved.position.x,
            y: moved.position.y,
        });
        step
    }

    fn initialize_possession_attraction(
        &mut self,
        slot: u16,
        projectile: Level2PossessionProjectile,
    ) {
        let rng = self
            .new_event_rng(slot)
            .wrapping_mul(9_377)
            .wrapping_add(9_439);
        self.mana_attraction_fields.push(Level2ManaAttractionField {
            source_entity_index: slot,
            entity_rng: rng,
            authored_index: u16::MAX,
            position: projectile.position,
            radius_squared: 12_845_056,
            max_life: 128,
            life: 128,
            action: MANA_ATTRACTION_ACTION,
            active: true,
        });
        self.source_phase_sidecars.insert(slot, slot as u8);
        self.source_rng.insert(slot, rng);
        self.source_owner
            .insert(slot, projectile.source_id_entity_index);
    }

    /// Source `PossesHitMana_320E0`: every live model-12 callback walks its
    /// complete map-search square and writes every qualifying mana/House
    /// mailbox through strict overlap, then advances phase and applies the
    /// signed `life-- < 0` terminal test.
    pub fn step_possession_effect_slot(
        &mut self,
        source_slot: u16,
        houses: &mut HouseFamily,
    ) -> bool {
        let Some(effect_index) = self
            .possession_effects
            .iter()
            .position(|effect| effect.active && effect.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut effect = self.possession_effects[effect_index];
        effect.phase = effect.phase.wrapping_add(1);
        let terminal = effect.life < 0;
        if !terminal {
            for candidate_slot in crate::level1_mission::source_map_search_slots(
                &self.map_chains,
                effect.position,
                &POSSESSION_IMPACT_SEARCH_OFFSETS,
            ) {
                // `sub_112D0(..., 0)` writes the source possession mailbox;
                // action52 consumes it on its own ordinary cursor visit.
                if let Some(house) = houses.houses().iter().find(|house| {
                    house.entity_index == candidate_slot
                        && house.active
                        && house.control_flags & 8 != 0
                        && !house.parameter.possession_blocked()
                        && house.source_id != effect.source_id_entity_index
                }) && possession_house_overlap(
                    house,
                    effect.position,
                    POSSESSION_IMPACT_COLLISION_HALF_WIDTH,
                    POSSESSION_IMPACT_COLLISION_VERTICAL_OFFSET,
                    POSSESSION_IMPACT_COLLISION_HALF_HEIGHT,
                ) {
                    houses
                        .submit_possession(candidate_slot, effect.source_id_entity_index, 0)
                        .expect("eligible live House accepts ordinary Possession mailbox");
                }
                let Some(sphere_index) = self.spheres.iter().position(|sphere| {
                    sphere.active
                        && !sphere.pending_delete
                        && sphere.source_entity_index == candidate_slot
                        && u16::try_from(sphere.id).ok() != Some(effect.source_id_entity_index)
                }) else {
                    continue;
                };
                let sphere = self.spheres[sphere_index];
                if sphere.overlaps_source_axes(
                    effect.position,
                    POSSESSION_IMPACT_COLLISION_HALF_WIDTH,
                    POSSESSION_IMPACT_COLLISION_HALF_WIDTH,
                    POSSESSION_IMPACT_COLLISION_VERTICAL_OFFSET,
                    POSSESSION_IMPACT_COLLISION_HALF_HEIGHT,
                ) {
                    let sphere = &mut self.spheres[sphere_index];
                    sphere.incoming_owner_entity_index = effect.source_id_entity_index;
                    sphere.incoming_possession_payload = 0;
                }
            }
        }
        effect.life = effect.life.wrapping_sub(1);
        if terminal {
            effect.active = false;
        }
        self.possession_effects[effect_index] = effect;
        // `DisableEntityDrawing04_57F10` marks a row hidden but leaves its
        // map relation and arena slot intact until the next `UpdateEntities`
        // prologue.  `reclaim_possession_rows_at_source_boundary` alone
        // performs that unlink/release.
        effect.active
    }

    /// Source `UpdateEntities_57730` performs the later `sub_57F20` unlink
    /// and free before the next pass begins. Possession landing callbacks may
    /// clear their draw/live state during a pass, but their source slot is not
    /// reusable until this one narrowly scoped prologue sweep.
    pub(super) fn reclaim_possession_rows_at_source_boundary(&mut self) {
        let reclaimed_projectiles = self
            .possession_projectiles
            .iter()
            .filter(|projectile| !projectile.active)
            .map(|projectile| {
                (
                    projectile.source_entity_index,
                    projectile.entity_rng,
                    projectile.position,
                )
            })
            .collect::<Vec<_>>();
        self.possession_projectiles
            .retain(|projectile| projectile.active);
        for (slot, entity_rng, position) in reclaimed_projectiles {
            self.possession_guidance_stale_records
                .insert(slot, position);
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }

        let reclaimed_effects = self
            .possession_effects
            .iter()
            .filter(|effect| !effect.active)
            .map(|effect| (effect.source_entity_index, effect.entity_rng))
            .collect::<Vec<_>>();
        self.possession_effects.retain(|effect| effect.active);
        for (slot, entity_rng) in reclaimed_effects {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
    }

    /// Source `sub_57F20` reclaim for hidden class-9/model-0 Fireballs.  The
    /// callback leaves the disabled row linked for the remainder of the pass;
    /// only this next source boundary unlinks it and returns its slot.
    pub(crate) fn reclaim_fireball_rows_at_source_boundary(&mut self) {
        let reclaimed = self
            .fireball_projectiles
            .iter()
            .filter(|projectile| !projectile.active && projectile.drawing_disabled)
            .map(|projectile| (projectile.source_entity_index, projectile.entity_rng))
            .collect::<Vec<_>>();
        self.fireball_projectiles
            .retain(|projectile| projectile.active || !projectile.drawing_disabled);
        for (slot, entity_rng) in reclaimed {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.source_owner.remove(&slot);
            self.release_slot(slot, Some(entity_rng));
        }
    }

    /// Source `TransformEntityToManaSphere_36BA0` with `use_mana_fraction`
    /// false. Allocation failure occurs after the dying source RNG write and
    /// does not consume a free slot or create a partial sphere.
    pub fn spawn_mana_sphere_from_death(
        &mut self,
        source_entity_index: u16,
        position: Position,
        source_yaw: i16,
        source_rng_after_death: u16,
        mana: i32,
        owner_entity_index: u16,
        terrain: &SessionTerrain,
    ) -> Option<u32> {
        self.spawn_mana_spheres_from_death(
            source_entity_index,
            position,
            source_yaw,
            source_rng_after_death,
            mana,
            owner_entity_index,
            false,
            terrain,
        )
        .into_iter()
        .next()
    }

    /// Source `TransformEntityToManaSphere_36BA0`. With `use_mana_fraction`
    /// enabled, the pinned C++ shadows its outer `manaFraction`: the loop still
    /// creates one sphere containing the full remainder. Preserve that behavior,
    /// including the dying RNG/owner writes when allocation is exhausted.
    pub fn spawn_mana_spheres_from_death(
        &mut self,
        source_entity_index: u16,
        position: Position,
        source_yaw: i16,
        source_rng_after_death: u16,
        mana: i32,
        owner_entity_index: u16,
        use_mana_fraction: bool,
        terrain: &SessionTerrain,
    ) -> Vec<u32> {
        if mana <= 0 {
            return Vec::new();
        }
        self.source_rng
            .insert(source_entity_index, source_rng_after_death);
        // TransformEntityToManaSphere_36BA0 clears the dying source's owner
        // relation after the sphere loop, even when the arena is exhausted.
        self.source_owner.insert(source_entity_index, 0);
        let fraction = crate::combat::actors::death_mana_sphere_count(mana, use_mana_fraction);
        let mut sphere_ids = Vec::with_capacity(fraction);
        for _ in 0..fraction {
            let Some(sphere_slot) = self.allocate_slot() else {
                break;
            };
            sphere_ids.push(self.initialize_death_mana_sphere_at_slot(
                sphere_slot,
                source_entity_index,
                position,
                source_yaw,
                mana,
                owner_entity_index,
                terrain,
            ));
        }
        sphere_ids
    }

    /// Initialize the model-39 death packet in a slot already allocated by the
    /// canonical source arena. Allocation and parent alias resolution belong
    /// to the caller; this constructor consumes only the child's RNG stream.
    pub(crate) fn initialize_death_mana_sphere_at_slot(
        &mut self,
        sphere_slot: u16,
        source_entity_index: u16,
        position: Position,
        source_yaw: i16,
        mana: i32,
        owner_entity_index: u16,
        terrain: &SessionTerrain,
    ) -> u32 {
        let sphere_seed = self.new_event_rng(sphere_slot);
        let sphere_rng = level2_next_rng16(sphere_seed);
        let yaw =
            i16::try_from((i32::from(sphere_rng % 0x71) + i32::from(source_yaw) - 56) & 0x07ff)
                .expect("Level 2 mana sphere yaw fits 11 bits");
        let speed_rng = level2_next_rng16(sphere_rng);
        let speed = i16::try_from(speed_rng % 0x30 + 16).expect("mana sphere speed fits i16");
        let mut velocity = Position::default();
        crate::move_entity(&mut velocity, yaw, 0, speed);
        let ground = terrain.ground_height(position);
        let height_above_ground = i32::from(position.z) - i32::from(ground);
        let vertical_velocity = level2_mana_sphere_vertical_velocity(height_above_ground);
        let sphere_mana = mana;
        let sphere = ManaSphere {
            id: u32::from(sphere_slot),
            source_entity_index: sphere_slot,
            action: 41,
            entity_rng: speed_rng,
            position,
            velocity_x: velocity.x.cast_signed(),
            velocity_y: velocity.y.cast_signed(),
            vertical_velocity,
            yaw,
            max_life: 300,
            life: 300,
            mana: sphere_mana,
            particle_index: 53,
            collision_yaw: 28,
            collision_pitch: 28,
            collision_roll: 28,
            collision_fov: 28,
            owner_entity_index,
            player_entity_index: owner_entity_index,
            incoming_owner_entity_index: 0,
            incoming_possession_payload: 0,
            ordinary_damage_payload: 0,
            ordinary_damage_owner: 0,
            attraction_source_entity_index: 0,
            attraction_step: 0,
            attraction_distance_high: 0,
            assigned_carrier_id: 0,
            assigned_defender_slot: 0,
            // NewEvent's damageable bit8 survives AddEventToMap's bit4.
            control_flags: 12,
            phase: sphere_slot as u8,
            active: true,
            pending_delete: false,
        };
        self.spheres.push(sphere);
        self.refresh_owner_mana_rows();
        self.events.push(Level2EconomyEvent::ManaSphereSpawned {
            sphere_id: sphere.id,
            source_entity_index,
            mana: sphere_mana,
        });
        sphere.id
    }

    /// Source `KillEntity_1C930` allocates a class-10/model-1/action-1
    /// death-burst row immediately after `TransformEntityToManaSphere` unless
    /// the dying entity's suppression bit is set.  The Level 2 Goat seam
    /// supplies the same ordered allocation through this production economy
    /// method; the burst's one-tick lifetime is consumed by its own dynamic
    /// source handler below.
    pub fn spawn_death_burst_from_death(
        &mut self,
        parent_entity_index: u16,
        position: Position,
    ) -> Option<u16> {
        let id_entity_index = self.source_owner(parent_entity_index);
        self.spawn_death_burst_from_death_with_id(parent_entity_index, id_entity_index, position)
    }

    /// Variant used by typed creature families that retain the source
    /// `id_0x1A` independently from their owner/player relation.
    pub fn spawn_death_burst_from_death_with_id(
        &mut self,
        parent_entity_index: u16,
        id_entity_index: u16,
        position: Position,
    ) -> Option<u16> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let burst = Level2DeathBurst {
            source_entity_index,
            entity_rng,
            parent_entity_index,
            id_entity_index,
            position,
            max_life: PLAYER_DEATH_BURST_MAX_LIFE,
            life_ticks: 1,
            subspell_index: PLAYER_DEATH_BURST_SUBSPELL_INDEX,
            // NewAdd0A01 clears damageability, sets replacement eligibility,
            // and AddEventToMap publishes map membership before dispatch.
            control_flags: SOURCE_REPLACEABLE_FLAG | 4,
            source_byte_3e: source_entity_index as u8,
            active: true,
        };
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .expect("Level 2 death-burst source slot fits the map chain");
        self.death_bursts.push(burst);
        self.events.push(Level2EconomyEvent::DeathBurstSpawned {
            source_entity_index,
            parent_entity_index,
            id_entity_index,
        });
        debug_assert_eq!(
            (DEATH_BURST_CLASS, DEATH_BURST_MODEL, DEATH_BURST_ACTION),
            (10, 1, 1)
        );
        Some(source_entity_index)
    }

    pub fn spawn_player_dying_burst(
        &mut self,
        player_entity_index: u16,
        player_source_id: u16,
        position: Position,
    ) -> Option<u16> {
        let source_slot = self.spawn_death_burst_from_death_with_id(
            player_entity_index,
            player_source_id,
            position,
        )?;
        let burst = self
            .death_bursts
            .iter_mut()
            .find(|burst| burst.source_entity_index == source_slot)
            .expect("new player death burst remains in the live arena");
        // `sub_5E310` adds byte[0] bit 0x80 after the generic model-1
        // constructor, linking the player-death dynamic light.
        burst.control_flags |= 0x80;
        Some(source_slot)
    }

    /// Allocate remc2's class-10/model-40/action-42 player death marker.
    /// The caller owns the action-2 retry rule: a failed allocation changes
    /// neither the player action nor any model-39 owner relation.
    pub fn allocate_player_death_marker(
        &mut self,
        trigger_player_entity_index: u16,
        position: Position,
    ) -> Option<Level2PlayerDeathMarker> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let marker = Level2PlayerDeathMarker {
            source_entity_index,
            entity_rng,
            trigger_player_entity_index,
            position,
            dispatch_phase: source_entity_index as u8,
            constructor_phase: u32::from(source_entity_index % 11),
            byte_38: crate::level1_mission::PLAYER_DEATH_MARKER_BYTE_38,
            max_life: crate::level1_mission::PLAYER_DEATH_MARKER_LIFE,
            life: crate::level1_mission::PLAYER_DEATH_MARKER_LIFE,
            control_flags: 12,
            particle_parameter_index: crate::level1_mission::PLAYER_DEATH_MARKER_PARTICLE_PARAMETER,
            active: true,
        };
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        self.source_owner.insert(source_entity_index, 0);
        self.player_death_markers.push(marker);
        Some(marker)
    }

    /// Rebind only live model-39 rows that still name the dying player in
    /// source `playerEntityIndex_0x94`, after marker allocation succeeds.
    pub fn redirect_player_mana_spheres_to_death_marker(
        &mut self,
        player_entity_index: u16,
        marker_entity_index: u16,
    ) {
        for sphere in self.spheres.iter_mut().filter(|sphere| {
            sphere.active
                && !sphere.pending_delete
                && sphere.player_entity_index == player_entity_index
        }) {
            sphere.player_entity_index = marker_entity_index;
        }
        self.refresh_owner_mana_rows();
    }

    /// Current reached action-42 state has no word-0x68 handoff relation, so
    /// `sub_36AE0` performs only its unconditional terrain grounding write.
    pub fn step_player_death_marker_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(marker) = self
            .player_death_markers
            .iter_mut()
            .find(|marker| marker.active && marker.source_entity_index == source_slot)
        else {
            return false;
        };
        marker.position.z = terrain.ground_height(marker.position);
        marker.dispatch_phase = marker.dispatch_phase.wrapping_add(1);
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: marker.position.x,
                y: marker.position.y,
            })
            .is_ok()
    }

    /// Advance the source class-10/model-1/action-1 burst.  ApplyEvents'
    /// model-1 fallthrough unconditionally disables the row after its action
    /// callback, so one live handler visit is the complete source lifetime.
    pub fn step_death_burst_slot(&mut self, source_slot: u16) -> bool {
        let Some(burst) = self
            .death_bursts
            .iter_mut()
            .find(|burst| burst.active && burst.source_entity_index == source_slot)
        else {
            return false;
        };
        burst.life_ticks = burst.life_ticks.wrapping_sub(1);
        burst.source_byte_3e = burst.source_byte_3e.wrapping_add(1);
        burst.active = false;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: false,
                x: burst.position.x,
                y: burst.position.y,
            })
            .expect("Level 2 death-burst source slot fits the map chain");
        self.events.push(Level2EconomyEvent::DeathBurstExpired {
            source_entity_index: source_slot,
            parent_entity_index: burst.parent_entity_index,
        });
        true
    }

    /// Return model-1 rows disabled by the preceding source pass. The slot is
    /// not reusable during the callback that hid it; the next pass boundary
    /// performs the source `sub_57F20` unlink/release.
    pub(crate) fn reclaim_death_burst_rows_at_source_boundary(&mut self) {
        let reclaimed = self
            .death_bursts
            .iter()
            .filter(|burst| !burst.active || burst.control_flags & ENTITY_DRAW_DISABLED_BIT != 0)
            .map(|burst| (burst.source_entity_index, burst.entity_rng))
            .collect::<Vec<_>>();
        self.death_bursts.retain(|burst| {
            burst.active && burst.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
        });
        for (slot, entity_rng) in reclaimed {
            self.release_slot(slot, Some(entity_rng));
        }
    }

    /// Apply the generation-time `ApplyEvents_498A0` terminal branch to one
    /// typed Castle child. The source does not dispatch a model callback for
    /// class-5/model-15 or class-10/model-41/42/43/79 rows in the terminal
    /// matrix: it sets the drawing disable bit, advances byte 0x3E for the
    /// class-10 row, and immediately calls `sub_57F20`, which unlinks the map
    /// row and returns the entity to the `NewEvent_4A050` free stack. A live
    /// model-79/action-0x56 row is handled separately when it is dispatched
    /// through its action table; this method represents the terminal matrix.
    pub fn applyevents_terminal_reclaim_castle_child(
        &mut self,
        source_slot: u16,
    ) -> Option<Level2CastleChild> {
        let Some(child_index) = self.castle_children.iter().position(|child| {
            child.active
                && child.source_entity_index == source_slot
                && child.source_class == 10
                && matches!(child.model, 41 | 42 | 43 | 79)
        }) else {
            return None;
        };
        let mut child = self.castle_children.remove(child_index);
        // ApplyEvents advances byte_0x3E_62 for class-10 terminal rows before
        // the same-visit sub_57F20 reclaim. Preserve that source write even
        // though the row is immediately returned to the free stack.
        if child.source_class == 10 {
            child.source_byte_3e = child.source_byte_3e.wrapping_add(1);
        }
        child.control_flags |= ENTITY_DRAW_DISABLED_BIT;
        child.active = false;
        self.sync_castle_child_map(child);
        self.release_slot(child.source_entity_index, Some(child.entity_rng));
        Some(child)
    }

    pub(crate) fn allocate_campaign_castle_defender(
        &mut self,
        authored_index: u16,
        position: Position,
        max_life: i32,
    ) -> Option<creatures::creature_15::CastleDefenderState> {
        let slot = self.allocate_slot()?;
        let rng = self.new_event_rng(slot);
        let mut actor = creatures::creature_15::CastleDefenderState::construct(
            slot,
            self.slot_generation(slot),
            rng,
            position,
            &mut self.castle_auxiliary_constructor_phase,
            creatures::creature_15::DefenderOrigin::AuthoredRecord(authored_index),
        );
        actor.max_life = max_life;
        actor.life = max_life;
        self.source_owner.insert(slot, 0);
        self.source_rng.insert(slot, rng);
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: true,
                x: position.x,
                y: position.y,
            })
            .expect("authored Castle defender source slot fits map chain");
        self.castle_defenders.push(actor);
        Some(actor)
    }

    pub(crate) fn castle_auxiliary_membership(
        &self,
        entities: &[EntitySnapshot],
    ) -> Level2CastleAuxiliaryMembership {
        let targets = creatures::creature_15::mc2::targets_in_economy(self, entities);
        let mut active_slots = entities
            .iter()
            .filter(|row| row.active)
            .map(|row| row.index)
            .collect::<BTreeSet<_>>();
        active_slots.extend(self.live_arena_slots().into_iter().map(|(slot, _)| slot));
        let handles = |eligible: fn(&creatures::creature_15::DefenderTarget) -> bool| {
            targets
                .iter()
                .filter(|row| active_slots.contains(&row.slot) && row.life >= 0 && eligible(row))
                .map(|row| crate::campaign::source_arena::SourceEntityHandle {
                    source_slot: row.slot,
                    generation: self.slot_generation(row.slot),
                })
                .collect::<Vec<_>>()
        };
        Level2CastleAuxiliaryMembership {
            replacement_list_epoch: self.replacement_list_epoch,
            class3_source_slots: handles(|row| row.class == 3),
            model15_source_slots: handles(|row| {
                row.class == 5 && row.model == 15 && !matches!(row.action, 180 | 232 | 234)
            }),
            class5_source_slots: handles(|row| {
                row.class == 5 && !matches!(row.action, 180 | 232 | 234)
            }),
        }
    }

    pub(crate) fn finish_castle_auxiliary_source_visit(&mut self, source_slot: u16) -> bool {
        if let Some(actor) = self
            .castle_defenders
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == source_slot)
        {
            actor.finish_source_visit();
            let actor = *actor;
            self.source_rng.insert(source_slot, actor.entity_rng);
            return self
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: true,
                    x: actor.position.x,
                    y: actor.position.y,
                })
                .is_ok();
        }
        if let Some(arrow) = self
            .authored_class5_archer_children
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == source_slot)
        {
            arrow.source_byte_3e = arrow.source_byte_3e.wrapping_add(1);
            let arrow = *arrow;
            self.source_rng.insert(source_slot, arrow.entity_rng);
            return self
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: true,
                    x: arrow.position.x,
                    y: arrow.position.y,
                })
                .is_ok();
        }
        if let Some(burst) = self
            .death_bursts
            .iter_mut()
            .find(|row| row.source_entity_index == source_slot)
        {
            burst.source_byte_3e = burst.source_byte_3e.wrapping_add(1);
            self.source_rng.insert(source_slot, burst.entity_rng);
            return true;
        }
        // Expiry may replace the current row with a class-10 effect. Its
        // source epilogue belongs to that current occupant, not the old actor.
        if let Some(effect) = self
            .model17_spawned_children
            .iter_mut()
            .find(|row| row.source_entity_index == source_slot)
        {
            effect.phase = effect.phase.wrapping_add(1);
            return true;
        }
        self.advance_source_phase_sidecar(source_slot).is_some()
    }

    pub(crate) fn step_castle_defender_with_context(
        &mut self,
        source_slot: u16,
        context: creatures::creature_15::DefenderContext<'_>,
    ) -> Option<creatures::creature_15::DefenderEffects> {
        creatures::creature_15::mc2::step_economy(self, source_slot, context)
    }

    fn step_castle_auxiliary_slot(
        &mut self,
        source_slot: u16,
        entities: &mut [EntitySnapshot],
        membership: &Level2CastleAuxiliaryMembership,
        terrain: &SessionTerrain,
    ) -> bool {
        let targets = creatures::creature_15::mc2::targets_in_economy(self, entities);
        let (class3, model15, class5) = self.castle_auxiliary_live_memberships(membership);
        let context = creatures::creature_15::DefenderContext {
            terrain,
            targets: &targets,
            class3: &class3,
            model15: &model15,
            class5: &class5,
            stage_runtime: None,
        };
        let Some(effects) = self.step_castle_defender_with_context(source_slot, context) else {
            return false;
        };
        if let Some(target) = effects
            .target_music
            .and_then(|slot| entities.get_mut(usize::from(slot)))
        {
            target.apply_source_sub_5ef70();
        }
        self.finish_castle_auxiliary_source_visit(source_slot)
    }

    pub(crate) fn castle_auxiliary_live_memberships(
        &self,
        membership: &Level2CastleAuxiliaryMembership,
    ) -> (Vec<u16>, Vec<u16>, Vec<u16>) {
        let slots = |handles: &[crate::campaign::source_arena::SourceEntityHandle]| {
            if membership.replacement_list_epoch != self.replacement_list_epoch {
                return Vec::new();
            }
            handles
                .iter()
                .filter(|handle| self.slot_generation(handle.source_slot) == handle.generation)
                .map(|handle| handle.source_slot)
                .collect::<Vec<_>>()
        };
        (
            slots(&membership.class3_source_slots),
            slots(&membership.model15_source_slots),
            slots(&membership.class5_source_slots),
        )
    }

    /// Advance one dynamic Castle child through its source callback or the
    /// terminal no-callback branch. Model 79/action 0x56 is the live
    /// `sub_3AF00` Castle-defend callback; model 42/action 44 is the
    /// BUILD-backed 37BC0 callback; model 41/action 43 remains the separate
    /// MSPRD-backed 377F0 callback. Other terminal rows retain source
    /// disable/reclaim.
    pub(crate) fn step_castle_child_terminal_slot(
        &mut self,
        source_slot: u16,
        entities: &mut [EntitySnapshot],
        auxiliary_membership: &Level2CastleAuxiliaryMembership,
        terrain: &mut SessionTerrain,
    ) -> bool {
        if self
            .castle_defenders
            .iter()
            .any(|row| row.active && row.source_entity_index == source_slot)
        {
            return self.step_castle_auxiliary_slot(
                source_slot,
                entities,
                auxiliary_membership,
                terrain,
            );
        }
        let Some(child_index) = self.castle_children.iter().position(|child| {
            child.active
                && child.source_entity_index == source_slot
                && ((child.source_class == 5 && child.model == 15)
                    || (child.source_class == 10 && matches!(child.model, 41 | 42 | 43 | 79)))
        }) else {
            return false;
        };
        let child = self.castle_children[child_index];
        if child.control_flags & ENTITY_DRAW_DISABLED_BIT != 0 {
            // `DisableEntityDrawing04_57F10` leaves the row for the generic
            // `UpdateEntities_57730` hidden-row prepass. That prepass runs
            // before the next dispatch walk, independently of this root.
            return true;
        }
        if child.source_class == 10 && child.model == 79 && child.action == 0x56 {
            return self.step_castle_controller_slot(child_index, terrain);
        }
        if child.source_class == 10 && child.model == 43 && child.action == 45 {
            return self.step_castle_upgrade_product(child_index);
        }
        if child.source_class == 10 && child.model == 42 && child.action == 44 {
            let live = self.step_castle_build_child(child_index, terrain);
            // `UpdateEntities_57730` performs the ordinary callback epilogue
            // after 37BC0 returns, including the byte_0x3E_62 increment.
            if let Some(child) = self.castle_children.get_mut(child_index) {
                child.source_byte_3e = child.source_byte_3e.wrapping_add(1);
            }
            return live;
        }
        if child.source_class == 10 && child.model == 41 && child.action == 43 {
            if level2_castle_model41_msprd_rect(child.cave_collision_cooldown).is_some() {
                let live = self.step_castle_msprd_child(child_index, terrain);
                // MSPRD uses the same ordinary source callback epilogue as
                // BUILD; hidden-row reclaim is a later source boundary and
                // must not advance this byte a second time.
                if let Some(child) = self.castle_children.get_mut(child_index) {
                    child.source_byte_3e = child.source_byte_3e.wrapping_add(1);
                }
                return live;
            }
            return true;
        }
        self.castle_children[child_index].control_flags |= ENTITY_DRAW_DISABLED_BIT;
        self.applyevents_terminal_reclaim_castle_child(source_slot)
            .is_some()
    }

    /// Reclaim every hidden Castle row at the ordinary source boundary.
    /// `UpdateEntities_57730` scans the whole entity arena before rebuilding
    /// lists or dispatching a root, so this cannot depend on a live Castle
    /// root, model, parent relation, or owner relation. Live BUILD/MSPRD,
    /// controller, defender, auxiliary, and projectile callbacks have already
    /// performed their same-pass epilogue; this sweep only performs the
    /// following-pass `sub_57F20` unlink/free and must not advance byte 0x3E
    /// again. A disabled Castle flyer remains an arena/map-tracked row until
    /// this boundary, just like a hidden Castle child.
    pub(super) fn reclaim_hidden_castle_rows_at_source_boundary(&mut self) {
        let hidden_slots: Vec<u16> = self
            .castle_children
            .iter()
            .filter(|child| child.active && child.control_flags & ENTITY_DRAW_DISABLED_BIT != 0)
            .map(|child| child.source_entity_index)
            .collect();
        let had_hidden_rows = !hidden_slots.is_empty();
        for source_slot in hidden_slots {
            let Some(child_index) = self
                .castle_children
                .iter()
                .position(|child| child.active && child.source_entity_index == source_slot)
            else {
                continue;
            };
            let mut child = self.castle_children.remove(child_index);
            child.active = false;
            self.sync_castle_child_map(child);
            self.release_slot(child.source_entity_index, Some(child.entity_rng));
        }

        let hidden_projectile_slots: Vec<(u16, u16)> = self
            .castle_projectiles
            .iter()
            .filter(|projectile| !projectile.active && projectile.drawing_disabled)
            .map(|projectile| (projectile.source_entity_index, projectile.entity_rng))
            .collect();
        let had_hidden_projectiles = !hidden_projectile_slots.is_empty();
        self.castle_projectiles
            .retain(|projectile| projectile.active || !projectile.drawing_disabled);
        for (source_slot, entity_rng) in hidden_projectile_slots {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.source_owner.remove(&source_slot);
            self.release_slot(source_slot, Some(entity_rng));
        }
        if had_hidden_rows || had_hidden_projectiles {
            self.refresh_owner_mana_rows();
        }
    }

    /// Bounded Level 2 slice of `sub_3AF00_castle_defend_event` for the live
    /// stage-1 model-79/action-0x56 controller. The first callback captures
    /// the pre-hover axis and applies the source signed six-unit height
    /// adjustment without RNG or terrain-material writes. The second callback
    /// advances the source RNG once and arms the source dword-10 timer. The
    /// later defense-spell/chain branches remain outside this slice, but the
    /// live row stays map-linked and source-owned throughout.
    fn step_castle_controller_slot(
        &mut self,
        child_index: usize,
        terrain: &SessionTerrain,
    ) -> bool {
        let child = self.castle_children[child_index];
        if child.life < 0 || child.id_entity_index == 0 {
            let child = {
                let child = &mut self.castle_children[child_index];
                child.source_byte_3e = child.source_byte_3e.wrapping_add(1);
                child.control_flags |= ENTITY_DRAW_DISABLED_BIT;
                child.active = false;
                *child
            };
            self.sync_castle_child_map(child);
            return false;
        }
        if matches!(child.cave_collision_cooldown, 9 | 0x0a) {
            let child = {
                let child = &mut self.castle_children[child_index];
                child.source_byte_3e = child.source_byte_3e.wrapping_add(1);
                child.control_flags |= ENTITY_DRAW_DISABLED_BIT;
                child.active = false;
                *child
            };
            self.sync_castle_child_map(child);
            return false;
        }

        {
            let child = &mut self.castle_children[child_index];
            match child.cave_collision_cooldown {
                0 => {
                    // `sub_3AF00` case 0 stores axis_0x9A before the
                    // callback-time hover adjustment.
                    child.axis_position = child.position;
                    child.cave_collision_cooldown = 1;
                }
                1 => {
                    // Cases 1 -> LABEL_9 use the newly advanced source RNG,
                    // then immediately decrement the 16..63 timer once.
                    child.entity_rng = child.entity_rng.wrapping_mul(9_377).wrapping_add(9_439);
                    child.cave_collision_cooldown = 2;
                    child.source_dword_10 = i32::from(child.entity_rng % 0x30) + 15;
                }
                2 => {
                    child.source_dword_10 = child.source_dword_10.wrapping_sub(1);
                    if child.source_dword_10 == 0 {
                        child.cave_collision_cooldown = 3;
                    }
                }
                _ => {}
            }

            let ground = terrain.ground_height(child.position);
            let clearance = if child.source_word_4a <= 1 { 384 } else { 224 };
            let target = ground.saturating_add(clearance);
            if child.position.z < target {
                child.position.z = target;
            } else if child.source_byte_3e & 8 != 0 {
                child.position.z = child.position.z.wrapping_add(6);
            } else {
                child.position.z = child.position.z.wrapping_sub(6);
            }

            // ApplyEvents increments byte_0x3E after the callback, so the
            // height branch above sees the pre-increment callback byte.
            child.source_byte_3e = child.source_byte_3e.wrapping_add(1);
        }
        let child = self.castle_children[child_index];
        self.sync_castle_child_map(child);
        true
    }

    fn step_castle_upgrade_product(&mut self, child_index: usize) -> bool {
        let child = self.castle_children[child_index];
        let hit = self.castle_at(child.parent_entity_index).is_some_and(|castle| {
            castle.source_class == 3
                && castle.source_model == 2
                && level2_model76_sub106c0_matches(
                    castle.position,
                    castle.shift_yaw,
                    castle.shift_pitch,
                    castle.shift_roll,
                    castle.shift_fov,
                    child.position,
                    child.yaw.cast_signed(),
                    child.roll.cast_signed(),
                    child.roll.cast_signed(),
                    child.roll.cast_signed(),
                )
        });
        let child = {
            let child = &mut self.castle_children[child_index];
            child.life = child.life.wrapping_sub(1);
            child.source_dword_10 = child.source_dword_10.wrapping_add(1);
            child.control_flags |= ENTITY_DRAW_DISABLED_BIT;
            // `sub_389F0` only calls DisableEntityDrawing04_57F10. The live
            // UpdateEntities row remains mapped until the next source-pass
            // prologue performs the later `sub_57F20` reclaim.
            child.control_flags |= 0x2;
            child.source_byte_3e = child.source_byte_3e.wrapping_add(1);
            child.active = true;
            *child
        };
        self.sync_castle_child_map(child);
        if let Some(castle) = self.castle_entry_mut(child.parent_entity_index) {
            if hit {
                castle.source_word_80 = child.owner_entity_index;
                castle.source_word_7c = 10;
            } else {
                castle.upgrade_holder_clear_pending = true;
            }
        }
        true
    }

    /// UpdateEntities' source-pass prologue reclaims a previously hidden
    /// class-10/model-43/action-45 upgrade product. The landing callback is
    /// intentionally live for the remainder of that pass so the row remains
    /// mapped and unavailable to the free stack until this boundary.
    pub fn reclaim_castle_upgrade_products_at_source_boundary(&mut self) {
        let mut reclaimed = Vec::new();
        self.castle_children.retain(|child| {
            let reclaim = child.active
                && child.source_class == 10
                && child.model == 43
                && child.action == 45
                && child.control_flags & ENTITY_DRAW_DISABLED_BIT != 0;
            if reclaim {
                reclaimed.push(*child);
            }
            !reclaim
        });
        for child in reclaimed {
            self.sync_castle_child_map(Level2CastleChild {
                active: false,
                ..child
            });
            self.release_slot(child.source_entity_index, Some(child.entity_rng));
        }
    }

    /// Source `AddTerrainMod0A_2A_37BC0` for model 42/action 44.  The
    /// callback owns only the BUILD timer and delegates every terrain write
    /// to `SessionTerrain::apply_castle_update`.
    fn composite_castle_construction_footprint(&self, stage: u8) -> Option<CastleBuildFootprint> {
        let current = self.castle_stage_footprints.get(&stage)?;
        let current_width = i32::from(current.width);
        let current_height = i32::from(current.height);
        let mut payload =
            vec![0xff_u8; usize::from(current.width) * usize::from(current.height) * 2];

        // 37BC0 clears a current-stage word buffer, then centers BUILD[1]
        // through BUILD[current stage] into it.  The qualified Level 2
        // records are square, while these explicit width/height offsets keep
        // the source tuple contract clear for later non-square records.
        for prior_stage in 1..=stage {
            let prior = self.castle_stage_footprints.get(&prior_stage)?;
            let row_offset = (current_height >> 1) - (i32::from(prior.height) >> 1);
            let column_offset = (current_width >> 1) - (i32::from(prior.width) >> 1);
            for source_row in 0..prior.height {
                for source_column in 0..prior.width {
                    let destination_row = row_offset + i32::from(source_row);
                    let destination_column = column_offset + i32::from(source_column);
                    if !(0..current_height).contains(&destination_row)
                        || !(0..current_width).contains(&destination_column)
                    {
                        return None;
                    }
                    let source_cell = prior.cells()[usize::from(source_row)
                        * usize::from(prior.width)
                        + usize::from(source_column)];
                    let destination = (usize::try_from(destination_row).ok()?
                        * usize::from(current.width)
                        + usize::try_from(destination_column).ok()?)
                        * 2;
                    // Material and height are separate source writes. A
                    // later ff component leaves the earlier stamped
                    // component in the composite, while a real component
                    // overwrites it. The ff/ff sentinel remains the existing
                    // apply_castle_update no-op for an untouched word.
                    if source_cell[0] != 0xff {
                        payload[destination] = source_cell[0];
                    }
                    if source_cell[1] != 0xff {
                        payload[destination + 1] = source_cell[1];
                    }
                }
            }
        }

        CastleBuildFootprint::from_owner_cells(
            current.index,
            current.width,
            current.height,
            &payload,
        )
        .ok()
    }

    fn step_castle_build_child(
        &mut self,
        child_index: usize,
        terrain: &mut SessionTerrain,
    ) -> bool {
        let child = self.castle_children[child_index];
        let Some(parent) = self.castle_at(child.parent_entity_index).copied() else {
            return true;
        };
        let Some(footprint) =
            self.composite_castle_construction_footprint(child.cave_collision_cooldown)
        else {
            return true;
        };

        let mut clock = child.source_dword_10;
        if child.control_flags & LEVEL2_CASTLE_CONSTRUCTION_INITIALIZED_BIT == 0 {
            clock = 19;
            self.castle_children[child_index].control_flags |=
                LEVEL2_CASTLE_CONSTRUCTION_INITIALIZED_BIT;
        }

        if clock <= 0 {
            clock = clock.wrapping_add(1);
            self.castle_children[child_index].source_dword_10 = clock;
            if clock == 0 {
                return self.complete_castle_construction(parent.source_entity_index, terrain);
            }
            return true;
        }

        clock = clock.wrapping_sub(1);
        if clock == 0 {
            clock = if child.source_byte_3b != 0 { -25 } else { -1 };
        } else if parent.source_word_30 == 0 {
            let remaining = i16::try_from(clock).expect("37BC0 BUILD timer fits i16");
            terrain
                .apply_castle_update(
                    CastleTerrainUpdate::ConstructionPass {
                        position: child.position,
                        remaining,
                    },
                    &footprint,
                )
                .expect("validated Castle BUILD footprint accepts construction pass");
        }
        self.castle_children[child_index].source_dword_10 = clock;
        true
    }

    /// Source `sub_377F0` for model 41/action 43.  This is intentionally kept
    /// separate from 37BC0: its timer and MSPRD geometry are not BUILD data.
    fn step_castle_msprd_child(
        &mut self,
        child_index: usize,
        terrain: &mut SessionTerrain,
    ) -> bool {
        let child = self.castle_children[child_index];
        let Some(parent) = self.castle_at(child.parent_entity_index).copied() else {
            return true;
        };
        let Some((width, height)) = level2_castle_model41_msprd_rect(child.cave_collision_cooldown)
        else {
            return true;
        };
        let center_x = child.position.x.wrapping_add(128).to_be_bytes()[0];
        let center_y = child.position.y.wrapping_add(128).to_be_bytes()[0];
        let start_x = center_x.wrapping_sub(width / 2);
        let start_y = center_y.wrapping_sub(height / 2);

        if child.control_flags & LEVEL2_CASTLE_CONSTRUCTION_INITIALIZED_BIT == 0 {
            let initial_height = child.position.z >> 5;
            let target_height = level2_castle_construction_height_from_square(
                &terrain.planes,
                start_x.wrapping_sub(1),
                start_y.wrapping_sub(1),
                width.wrapping_add(2),
                height.wrapping_add(2),
            )
            .min(220);
            let child = &mut self.castle_children[child_index];
            child.source_word_2e = initial_height;
            child.source_dword_10 = if initial_height == target_height {
                0
            } else {
                10
            };
            child.construction_target_height = target_height;
            child.control_flags |= LEVEL2_CASTLE_CONSTRUCTION_INITIALIZED_BIT;
            return true;
        }

        let clock = child.source_dword_10;
        if parent.source_word_30 == 0 && clock != 0 {
            let step = (i32::from(child.construction_target_height)
                - i32::from(child.source_word_2e))
                / clock;
            let step = i16::try_from(step).expect("Castle construction step fits i16");
            let next_height = child.source_word_2e.wrapping_add(step);
            let mut terrain_touched = false;
            if clock > 0 {
                terrain_touched = true;
                for row in 0..height {
                    for column in 0..width {
                        let index =
                            terrain_index(start_x.wrapping_add(column), start_y.wrapping_add(row));
                        terrain.planes.ground_heights[index] =
                            terrain.planes.ground_heights[index].wrapping_add(step as u8);
                        if clock == 1 {
                            let mut angle = terrain.planes.angles[index];
                            if angle & 0x80 != 0 {
                                angle &= !0x80;
                                if terrain.map_type == MapType::Day {
                                    angle |= 0x08;
                                }
                                terrain.planes.angles[index] = angle;
                            }
                        }
                    }
                }
            } else if clock == -1 && terrain.map_type == MapType::Day {
                for row in 0..height {
                    for column in 0..width {
                        let index =
                            terrain_index(start_x.wrapping_add(column), start_y.wrapping_add(row));
                        let angle = &mut terrain.planes.angles[index];
                        if *angle & 0x08 != 0 {
                            *angle = (*angle | 0x80) & !0x08;
                            terrain_touched = true;
                        }
                    }
                }
            }
            let next_clock = if clock == 1 {
                -10
            } else if clock == -1 {
                0
            } else if clock < 0 {
                clock + 1
            } else {
                clock - 1
            };
            let child = &mut self.castle_children[child_index];
            child.source_word_2e = next_height;
            child.source_dword_10 = next_clock;
            if terrain_touched {
                terrain.mutation_sequence = terrain.mutation_sequence.wrapping_add(1);
            }
            return true;
        }

        self.complete_castle_msprd_child(parent.source_entity_index, child, width, height, terrain);
        true
    }

    fn complete_castle_msprd_child(
        &mut self,
        parent_slot: u16,
        child: Level2CastleChild,
        width: u8,
        height: u8,
        terrain: &mut SessionTerrain,
    ) {
        if let Some(parent) = self.castle_entry_mut(parent_slot) {
            parent.source_word_2e = 2;
            parent.anchor_position.z = child.source_word_2e.saturating_mul(32);
        }
        let center_x = child.position.x.wrapping_add(128).to_be_bytes()[0];
        let center_y = child.position.y.wrapping_add(128).to_be_bytes()[0];
        let left = center_x.wrapping_sub(width / 2).wrapping_sub(3);
        let top = center_y.wrapping_sub(height / 2);
        for row in 0..height.saturating_sub(1) {
            for offset in 0..4 {
                smooth_building_height(
                    &mut terrain.planes,
                    left.wrapping_add(offset),
                    top.wrapping_add(row),
                    terrain.map_type,
                );
                smooth_building_height(
                    &mut terrain.planes,
                    center_x.wrapping_add(width / 2).wrapping_add(offset),
                    top.wrapping_add(row),
                    terrain.map_type,
                );
            }
        }
        for column in 0..width.saturating_add(6) {
            for offset in 0..4 {
                smooth_building_height(
                    &mut terrain.planes,
                    left.wrapping_add(column),
                    top.wrapping_sub(3).wrapping_add(offset),
                    terrain.map_type,
                );
                smooth_building_height(
                    &mut terrain.planes,
                    left.wrapping_add(column),
                    top.wrapping_add(height.saturating_sub(1))
                        .wrapping_add(offset),
                    terrain.map_type,
                );
            }
        }
        if let Some(child) = self
            .castle_children
            .iter_mut()
            .find(|candidate| candidate.source_entity_index == child.source_entity_index)
        {
            child.control_flags |= ENTITY_DRAW_DISABLED_BIT;
        }
    }

    pub fn claim_mana_sphere(&mut self, sphere_slot: u16, owner_entity_index: u16) -> bool {
        let Some(sphere) = self
            .spheres
            .iter_mut()
            .find(|sphere| sphere.active && sphere.source_entity_index == sphere_slot)
        else {
            return false;
        };
        if sphere.owner_entity_index == owner_entity_index {
            return false;
        }
        sphere.owner_entity_index = owner_entity_index;
        self.events.push(Level2EconomyEvent::ManaSphereClaimed {
            sphere_id: sphere.id,
            owner_entity_index,
        });
        self.refresh_owner_mana_rows();
        true
    }

    /// Advance one live Level 2 mana-sphere source row through the same
    /// gravity, ground-bounce, slope impulse, map-chain, and merge order as
    /// the authored Level 1 source action.  This is intentionally a small
    /// production handler: carrier attraction and possession mailbox input
    /// remain separate source actions, while this method makes the dynamic
    /// arena row itself stateful and reclaimable by the outer dispatcher.
    pub fn step_mana_sphere_slot(
        &mut self,
        sphere_slot: u16,
        terrain: &SessionTerrain,
        entities: &[crate::native_world::EntitySnapshot],
        color_base_for_owner: impl Fn(u16) -> u16,
    ) -> bool {
        let Some(sphere_index) = self
            .spheres
            .iter()
            .position(|sphere| sphere.active && sphere.source_entity_index == sphere_slot)
        else {
            return false;
        };
        // Source byte1 bit8 skips this entire callback body once, including
        // the color update; the common phase epilogue still runs.
        if self.spheres[sphere_index].control_flags & 0x800 != 0 {
            let sphere = &mut self.spheres[sphere_index];
            sphere.control_flags &= !0x800;
            sphere.phase = sphere.phase.wrapping_add(1);
            return true;
        }
        let owner_changed = self.spheres[sphere_index].incoming_owner_entity_index != 0
            && self.spheres[sphere_index].incoming_owner_entity_index
                != self.spheres[sphere_index].player_entity_index;
        // `TransformArcherToMana_35940` consumes the model-12 mailbox before
        // movement. A tier-0 pulse transfers only when the raw protected bit
        // is clear. The accepted source branch writes `word_0x68` into
        // `playerEntityIndex_0x94`; keep the semantic owner projection on the
        // same side so Castle defenders consume the authoritative relation.
        let claimed_owner = {
            let sphere = &mut self.spheres[sphere_index];
            let pending_owner = sphere.incoming_owner_entity_index;
            if pending_owner == 0 {
                None
            } else {
                let payload = sphere.incoming_possession_payload;
                sphere.incoming_owner_entity_index = 0;
                sphere.incoming_possession_payload = 0;
                let protected = sphere.control_flags & 0x0020_0000 != 0;
                // sub_35940 compares word68 with the source word94 relation,
                // which Nyphur can provisionally change before its pulse lands.
                if sphere.player_entity_index != pending_owner && (payload != 0 || !protected) {
                    sphere.owner_entity_index = pending_owner;
                    sphere.player_entity_index = pending_owner;
                    sphere.control_flags &= !0x40;
                    if payload != 0 {
                        sphere.control_flags |= 0x0020_0000;
                    }
                    Some((sphere.id, pending_owner))
                } else {
                    None
                }
            }
        };
        if let Some((sphere_id, owner_entity_index)) = claimed_owner {
            self.events.push(Level2EconomyEvent::ManaSphereClaimed {
                sphere_id,
                owner_entity_index,
            });
        }
        // `TransformArcherToMana_35940` consumes the source-linked attraction
        // mailbox before the ordinary sphere physics and clears it after one
        // movement attempt. A field that expired before this row's cursor
        // visit is treated as a stale relation and cleared without movement.
        let attraction = {
            let sphere = self.spheres[sphere_index];
            (
                sphere.attraction_source_entity_index,
                sphere.attraction_step,
            )
        };
        if attraction.0 != 0 {
            if let Some(field) = self
                .mana_attraction_fields
                .iter()
                .find(|field| field.active && field.source_entity_index == attraction.0)
            {
                let heading = i16::try_from(heading_for_positions(
                    self.spheres[sphere_index].position,
                    field.position,
                ))
                .expect("Level 2 attraction heading fits i16");
                let mut displacement = Position::default();
                crate::move_entity(
                    &mut displacement,
                    heading,
                    0,
                    i16::try_from(attraction.1).expect("Level 2 attraction step fits i16"),
                );
                let sphere = &mut self.spheres[sphere_index];
                sphere.position.x = sphere
                    .position
                    .x
                    .wrapping_add_signed(displacement.x.cast_signed());
                sphere.position.y = sphere
                    .position
                    .y
                    .wrapping_add_signed(displacement.y.cast_signed());
                sphere.yaw = heading;
            }
            let sphere = &mut self.spheres[sphere_index];
            sphere.attraction_source_entity_index = 0;
            sphere.attraction_step = 0;
            sphere.attraction_distance_high = 0;
        }

        // `TransformArcherToMana_35940` also consumes the class-3/model-3
        // carrier relation written by `AddBallon_60AB0`. While linked, the
        // sphere skips ordinary rolling physics, moves 16 source units toward
        // the exact carrier inside the 1,024-unit attraction radius, then
        // converges vertically in 32-unit steps once X/Y coincide. Without
        // this callback the carrier can schedule a valid target forever but
        // can never satisfy its strict 3D collection AABB at ground clearance.
        let assigned_carrier_slot = self.spheres[sphere_index].assigned_carrier_id;
        if assigned_carrier_slot != 0 {
            let carrier = u16::try_from(assigned_carrier_slot).ok().and_then(|slot| {
                self.carriers
                    .iter()
                    .find(|carrier| carrier.active && carrier.source_entity_index == slot)
                    .copied()
            });
            if let Some(carrier) = carrier {
                let sphere = &mut self.spheres[sphere_index];
                sphere.vertical_velocity = 128;
                sphere.yaw =
                    i16::try_from(heading_for_positions(sphere.position, carrier.position))
                        .expect("carrier-attracted mana-sphere heading fits");
                let distance = level2_wrapping_target_distance(sphere.position, carrier.position);
                if distance <= MANA_CARRIER_ATTRACTION_RADIUS {
                    if distance >= 16 {
                        crate::move_entity(
                            &mut sphere.position,
                            sphere.yaw,
                            0,
                            MANA_CARRIER_ATTRACTION_SPEED,
                        );
                    } else {
                        sphere.position.x = carrier.position.x;
                        sphere.position.y = carrier.position.y;
                        if sphere.position.z >= carrier.position.z {
                            if i32::from(sphere.position.z) > i32::from(carrier.position.z) + 512 {
                                sphere.position.z = sphere
                                    .position
                                    .z
                                    .wrapping_sub(MANA_CARRIER_ATTRACTION_VERTICAL_STEP);
                            }
                        } else {
                            sphere.position.z = sphere
                                .position
                                .z
                                .wrapping_add(MANA_CARRIER_ATTRACTION_VERTICAL_STEP);
                        }
                    }
                    let ground = terrain.ground_height(sphere.position);
                    if sphere.position.z < ground {
                        sphere.position.z = ground;
                    }
                } else {
                    sphere.assigned_carrier_id = 0;
                }
            } else {
                self.spheres[sphere_index].assigned_carrier_id = 0;
            }
            self.spheres[sphere_index].phase = self.spheres[sphere_index].phase.wrapping_add(1);
            let sphere = self.spheres[sphere_index];
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: sphere_slot,
                    active: true,
                    x: sphere.position.x,
                    y: sphere.position.y,
                })
                .expect("carrier-attracted mana-sphere source slot fits the map chain");
            self.refresh_owner_mana_rows();
            return true;
        }

        // Source `sub_35FB0` resolves word_0x96 through the raw entity table
        // before dispatching the holder-specific lift. Castle class3/model3
        // keeps its fixed 32-unit step; creature class5/model23 uses its raw
        // word_0x2C value at record bytes 48..50. A stale raw slot must not
        // be rescued by a matching typed sidecar.
        let defender_attracted = self.spheres[sphere_index].control_flags & 0x40 != 0;
        if defender_attracted {
            let defender_slot = self.spheres[sphere_index].assigned_defender_slot;
            let holder = (defender_slot != 0)
                .then(|| entities.get(usize::from(defender_slot)))
                .flatten()
                .filter(|entity| entity.active && entity.index == defender_slot);
            let holder = holder.and_then(|entity| match (entity.class, entity.model) {
                (3, 3) => self
                    .castle_children
                    .iter()
                    .find(|child| {
                        child.active
                            && child.source_entity_index == defender_slot
                            && child.source_class == 3
                            && child.model == 3
                    })
                    .map(|child| (child.position, 32_i16)),
                (5, 23) => Some((
                    entity.position,
                    i16::from_le_bytes([entity.record[48], entity.record[49]]),
                )),
                _ => None,
            });
            let mut position_changed = false;
            if let Some((holder_position, lift_step)) = holder {
                let sphere = &mut self.spheres[sphere_index];
                sphere.vertical_velocity = 128;
                sphere.yaw = i16::try_from(heading_for_positions(sphere.position, holder_position))
                    .expect("holder-attracted mana-sphere heading fits");
                let distance = level2_wrapping_target_distance(sphere.position, holder_position);
                if distance <= 1_024 {
                    let mut predicted = sphere.position;
                    if distance >= 16 {
                        crate::move_entity(&mut predicted, sphere.yaw, 0, 16);
                    } else {
                        predicted.x = holder_position.x;
                        predicted.y = holder_position.y;
                        if predicted.z >= holder_position.z {
                            if i32::from(predicted.z) > i32::from(holder_position.z) + 512 {
                                predicted.z = predicted.z.wrapping_sub(lift_step);
                            }
                        } else {
                            predicted.z = predicted.z.wrapping_add(lift_step);
                        }
                    }
                    let ground = terrain.ground_height(predicted);
                    if ground > predicted.z {
                        predicted.z = ground;
                    }
                    sphere.position = predicted;
                    position_changed = true;
                } else {
                    sphere.control_flags &= !0x40;
                }
            } else {
                self.spheres[sphere_index].control_flags &= !0x40;
            }

            if position_changed {
                let position = self.spheres[sphere_index].position;
                self.map_chains
                    .sync_entity(RenderableMapSeed {
                        entity_index: sphere_slot,
                        active: true,
                        x: position.x,
                        y: position.y,
                    })
                    .expect("holder-attracted sphere source slot fits the map chain");
            }
            let sphere = &mut self.spheres[sphere_index];
            if sphere.control_flags & LEVEL2_MANA_SPHERE_TARGET_SUPPRESSION_BIT != 0 {
                sphere.life = sphere.life.wrapping_sub(1);
                match sphere.life {
                    12 => sphere.control_flags |= 0x0100_0000,
                    6 => {
                        sphere.control_flags |= 0x0080_0000;
                        sphere.control_flags &= !0x0100_0000;
                    }
                    0 => sphere.control_flags |= ENTITY_DRAW_DISABLED_BIT,
                    _ => {}
                }
            }
            sphere.phase = sphere.phase.wrapping_add(1);
            self.refresh_owner_mana_rows();
            return true;
        }
        let grounded;
        {
            let sphere = &mut self.spheres[sphere_index];
            sphere.velocity_x = sphere
                .velocity_x
                .clamp(-MANA_SPHERE_MAX_XY_VELOCITY, MANA_SPHERE_MAX_XY_VELOCITY);
            sphere.velocity_y = sphere
                .velocity_y
                .clamp(-MANA_SPHERE_MAX_XY_VELOCITY, MANA_SPHERE_MAX_XY_VELOCITY);
            sphere.position.x = sphere.position.x.wrapping_add_signed(sphere.velocity_x);
            sphere.position.y = sphere.position.y.wrapping_add_signed(sphere.velocity_y);
            sphere.position.z = sphere.position.z.wrapping_add(sphere.vertical_velocity);
            sphere.vertical_velocity = sphere.vertical_velocity.wrapping_sub(MANA_SPHERE_GRAVITY);
            if sphere.vertical_velocity < MANA_SPHERE_TERMINAL_VELOCITY {
                sphere.vertical_velocity = MANA_SPHERE_TERMINAL_VELOCITY;
            }
            let ground = terrain.ground_height(sphere.position);
            if sphere.position.z < ground {
                sphere.position.z = ground;
                sphere.vertical_velocity = -(sphere.vertical_velocity / 4);
                if sphere.vertical_velocity <= 16 {
                    sphere.vertical_velocity = 0;
                }
            }
            grounded = sphere.position.z == ground;
            if grounded {
                let [_, tile_x] = sphere.position.x.to_le_bytes();
                let [_, tile_y] = sphere.position.y.to_le_bytes();
                let sample = |x: u8, y: u8| {
                    i32::from(terrain.ground_height(Position {
                        x: u16::from(x) << 8,
                        y: u16::from(y) << 8,
                        z: 0,
                    })) / 32
                };
                let height_00 = sample(tile_x, tile_y);
                let height_10 = sample(tile_x.wrapping_add(1), tile_y);
                let height_11 = sample(tile_x.wrapping_add(1), tile_y.wrapping_add(1));
                let height_01 = sample(tile_x, tile_y.wrapping_add(1));
                let slope_x = height_00 - height_10 - height_11 + height_01;
                let slope_y = height_00 + height_10 - height_11 - height_01;
                sphere.velocity_x = sphere
                    .velocity_x
                    .wrapping_add(i16::try_from(slope_x).expect("Level 2 slope impulse fits"));
                sphere.velocity_y = sphere
                    .velocity_y
                    .wrapping_add(i16::try_from(slope_y).expect("Level 2 slope impulse fits"));
                sphere.velocity_x = i16::try_from(i32::from(sphere.velocity_x) * 250 / 256)
                    .expect("Level 2 sphere velocity fits");
                sphere.velocity_y = i16::try_from(i32::from(sphere.velocity_y) * 250 / 256)
                    .expect("Level 2 sphere velocity fits");
            }
        }
        let position = self.spheres[sphere_index].position;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: sphere_slot,
                active: true,
                x: position.x,
                y: position.y,
            })
            .expect("Level 2 sphere source slot fits the map chain");

        let merge_suppressed = self.spheres[sphere_index].control_flags
            & LEVEL2_MANA_SPHERE_TARGET_SUPPRESSION_BIT
            != 0;
        if grounded && !merge_suppressed {
            let sphere = self.spheres[sphere_index];
            let absorbed_slot = level2_first_colliding_mana_sphere_slot(
                &self.map_chains,
                &self.spheres,
                sphere_slot,
                sphere,
            );
            if let Some(absorbed_slot) = absorbed_slot {
                self.merge_mana_spheres(sphere_slot, absorbed_slot);
            }
        }
        let sphere = self
            .spheres
            .iter_mut()
            .find(|sphere| sphere.active && sphere.source_entity_index == sphere_slot)
            .expect("the source sphere remains after absorbing a collision peer");
        // Source36920 follows collision/merge in the ordinary movement
        // branch, and is skipped while fading unless a new owner was offered.
        if !merge_suppressed || owner_changed {
            sphere.apply_particle_color(color_base_for_owner(sphere.player_entity_index));
        }
        if sphere.control_flags & LEVEL2_MANA_SPHERE_TARGET_SUPPRESSION_BIT != 0 {
            sphere.life = sphere.life.wrapping_sub(1);
            match sphere.life {
                12 => sphere.control_flags |= 0x0100_0000,
                6 => {
                    sphere.control_flags |= 0x0080_0000;
                    sphere.control_flags &= !0x0100_0000;
                }
                0 => sphere.control_flags |= ENTITY_DRAW_DISABLED_BIT,
                _ => {}
            }
        }
        sphere.phase = sphere.phase.wrapping_add(1);
        self.refresh_owner_mana_rows();
        true
    }

    /// Advance one live Castle model-3/action-9 holder through the proven
    /// class-3 branch of `AddBallon_60AB0`.  `sub_5FF50` installs the Castle
    /// root in the holder's raw `word_0x96_150`; the root `level` is the
    /// target `dword_0x10_16` range multiplier.  Sphere/class-10 targets and
    /// the deeper combat/spell callbacks remain deliberately outside this
    /// typed handler.
    pub fn step_castle_defender_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(child_index) = self.castle_children.iter().position(|child| {
            child.active
                && child.source_entity_index == source_slot
                && child.source_class == 3
                && child.model == 3
                && child.action == 9
        }) else {
            return false;
        };
        let child_snapshot = self.castle_children[child_index];
        if child_snapshot.control_flags & ENTITY_DRAW_DISABLED_BIT != 0 {
            // `TransformEntityToManaSphere(false)` disables the holder in
            // `sub_5FF50`; the source prologue reclaims it on the following
            // pass. Keep the hidden row out of the live callback body until
            // that boundary rather than moving or collecting it again.
            return true;
        }
        let mut predicted = child_snapshot.position;
        let mut yaw = child_snapshot.yaw as i16;
        let mut transfer_mana = None;
        let mut collected_sphere = None;
        let castle_snapshot = self.castle_at(child_snapshot.target_sphere_slot).copied();
        if let Some(castle) = castle_snapshot {
            yaw = i16::try_from(heading_for_positions(predicted, castle.position))
                .expect("Castle defender heading fits");
            let distance = level2_wrapping_target_distance(predicted, castle.position);
            let target_range = u32::from(castle.level);
            let threshold =
                u32::from(child_snapshot.speed.unsigned_abs()).saturating_mul(target_range);
            if distance <= threshold {
                // NewEvent_4A050 binds the holder's str_160 word_0xc to zero
                // for the default event profile.  This is the exact source
                // altitude gate before the class-3 mana transfer.
                let ground = terrain.ground_height(predicted);
                // `sub_4ABA0` binds `str_D7BD6[68].word_160_0xc_12` to 512;
                // the source compares the holder's predicted Z against that
                // terrain-relative upper bound, not against the ground
                // plane itself.
                if predicted.z <= ground.saturating_add(512) && castle.level > 0 {
                    transfer_mana = Some(child_snapshot.mana);
                    // The source snaps the holder's predicted axis to the
                    // class-3 target position; the target Castle itself does
                    // not move. Its mana receives the holder payload.
                    predicted.x = castle.position.x;
                    predicted.y = castle.position.y;
                    if let Some(target) = self.castle_at_mut(castle.source_entity_index) {
                        target.mana = target.mana.wrapping_add(child_snapshot.mana);
                    }
                }
            } else {
                crate::move_entity(&mut predicted, yaw, 0, child_snapshot.speed);
            }
        } else if let Some(sphere_index) = self.spheres.iter().position(|sphere| {
            sphere.active
                && !sphere.pending_delete
                && sphere.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
                && sphere.source_entity_index == child_snapshot.target_sphere_slot
        }) {
            // Source `AddBallon_60AB0` uses the defender's raw id relation for
            // the owner gate.  This is `source_id_entity_index`, not the
            // Castle's presentation/owner field.
            let sphere = self.spheres[sphere_index];
            yaw = i16::try_from(heading_for_positions(predicted, sphere.position))
                .expect("mana-sphere defender heading fits");
            let distance = level2_wrapping_target_distance(predicted, sphere.position);
            let attraction_limit = if terrain.map_type == MapType::Cave {
                2_048
            } else {
                1_024
            };
            // `AddBallon_60AB0` jumps directly to LABEL_20 when the sphere's
            // raw player relation does not match the holder id.  That path
            // still runs the shared settle tail, but it does not perform the
            // target-distance snap or `MoveEntity` step.
            let relation_matches = sphere.player_entity_index == child_snapshot.id_entity_index;
            if relation_matches {
                if distance > attraction_limit {
                    // The source clears byte-0 bit 0x40 when the target is
                    // outside the Day/Cave attraction range, then continues
                    // through the normal movement/settle tail.
                    if let Some(target) = self.spheres.get_mut(sphere_index) {
                        target.control_flags &= !0x40;
                    }
                } else {
                    if let Some(target) = self.spheres.get_mut(sphere_index) {
                        target.control_flags |= 0x40;
                        target.assigned_defender_slot = source_slot;
                    }
                    if level2_castle_defender_sphere_overlap(predicted, &sphere) {
                        collected_sphere = Some((sphere_index, sphere));
                    }
                }
                if distance <= u32::from(child_snapshot.speed.unsigned_abs()) {
                    predicted.x = sphere.position.x;
                    predicted.y = sphere.position.y;
                } else {
                    crate::move_entity(&mut predicted, yaw, 0, child_snapshot.speed);
                }
            }
        } else if child_snapshot.target_sphere_slot != 0 {
            // Clear a holder relation once its source row has disappeared;
            // the source does this on the next AddBallon call before settle.
            if let Some(target) = self
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == source_slot)
            {
                target.target_sphere_slot = 0;
            }
            for sphere in &mut self.spheres {
                if sphere.assigned_defender_slot == source_slot {
                    sphere.assigned_defender_slot = 0;
                }
            }
        }

        let mut cave_control_flags = child_snapshot.control_flags;
        let mut cave_speed = child_snapshot.speed;
        let mut cave_collision_cooldown = child_snapshot.cave_collision_cooldown;
        let mut cave_sound = false;
        let ground = terrain.ground_height(predicted);
        if terrain.map_type == MapType::Cave {
            // Pinned remc2 `sub_60D50` (EventsFunctions.cpp:61920-61976):
            // a Cave holder is blocked by the map angle bit 8 or by the
            // strict `sub_11E70` ceiling test.  The latter is
            // `fov + ground + str_D7BD6[68].word_0xc (512) > ceiling`.
            let tile = crate::Tile {
                x: predicted.x.to_be_bytes()[0],
                y: predicted.y.to_be_bytes()[0],
            };
            let angle_blocked = terrain.angle_flags(tile) & 8 != 0;
            let ceiling_blocked = terrain.ceiling_height(predicted).is_some_and(|ceiling| {
                i32::from(LEVEL2_CASTLE_DEFENDER_CAVE_FOV) + i32::from(ground) + 512
                    > i32::from(ceiling)
            });
            let blocked = angle_blocked || ceiling_blocked;
            let was_blocked = cave_control_flags & 1 != 0;
            cave_collision_cooldown = cave_collision_cooldown.saturating_sub(1);
            if blocked {
                cave_control_flags |= 1;
                cave_speed = LEVEL2_CASTLE_DEFENDER_CAVE_BLOCKED_SPEED;
            } else {
                cave_control_flags &= !1;
                cave_speed = LEVEL2_CASTLE_DEFENDER_CAVE_OPEN_SPEED;
            }
            if blocked != was_blocked && cave_collision_cooldown == 0 {
                cave_sound = true;
                cave_collision_cooldown = LEVEL2_CASTLE_DEFENDER_CAVE_SOUND_COOLDOWN;
            }

            // Exact `sub_580E0(a1, ground, word_c=512, word_a=1536,
            // word_e=-16)` order.  The source helper ignores its fourth
            // argument and uses a signed 16-bit comparison for the settle
            // branch, so the resulting floor is ground + 512.
            if predicted.z > ground {
                predicted.z = predicted.z.wrapping_add(LEVEL2_CASTLE_DEFENDER_SETTLE_STEP);
            }
            if i32::from(predicted.z)
                <= i32::from(ground) + i32::from(LEVEL2_CASTLE_DEFENDER_FLOOR_CLEARANCE)
            {
                predicted.z = ground.saturating_add(LEVEL2_CASTLE_DEFENDER_FLOOR_CLEARANCE);
            }
            if !blocked {
                if let Some(ceiling) = terrain.ceiling_height(predicted) {
                    let cap = ceiling.saturating_sub(LEVEL2_CASTLE_DEFENDER_CAVE_FOV);
                    if predicted.z > cap {
                        predicted.z = cap;
                    }
                }
            }
        } else {
            // `sub_4ABA0` binds every Castle defender to descriptor 68.
            // `AddBallon_60AB0` forwards that descriptor's c=512 and e=-16
            // words to `sub_580E0` on non-Cave maps.
            if predicted.z > ground {
                predicted.z = predicted.z.wrapping_add(LEVEL2_CASTLE_DEFENDER_SETTLE_STEP);
            }
            if i32::from(predicted.z)
                <= i32::from(ground) + i32::from(LEVEL2_CASTLE_DEFENDER_FLOOR_CLEARANCE)
            {
                predicted.z = ground.saturating_add(LEVEL2_CASTLE_DEFENDER_FLOOR_CLEARANCE);
            }
        }
        if let Some(mana) = transfer_mana {
            let child = &mut self.castle_children[child_index];
            child.position = predicted;
            child.yaw = yaw as u16;
            child.mana = 0;
            child.player_entity_index = child.id_entity_index;
            child.life = child.max_life;
            self.events.push(Level2EconomyEvent::CastleDefenderArrived {
                defender_slot: source_slot,
                castle_id: child_snapshot.target_sphere_slot,
                mana,
            });
        } else if let Some((sphere_index, sphere)) = collected_sphere {
            let child = &mut self.castle_children[child_index];
            child.position = predicted;
            child.yaw = yaw as u16;
            child.mana = child.mana.wrapping_add(sphere.mana);
            child.player_entity_index = sphere.player_entity_index;
            child.life = child.max_life;
            child.target_sphere_slot = 0;
            let sphere_state = &mut self.spheres[sphere_index];
            sphere_state.assigned_carrier_id = 0;
            sphere_state.assigned_defender_slot = 0;
            sphere_state.pending_delete = true;
            sphere_state.control_flags |= ENTITY_DRAW_DISABLED_BIT;
            self.events
                .push(Level2EconomyEvent::CastleDefenderCollectedSphere {
                    defender_slot: source_slot,
                    sphere_id: sphere.id,
                    mana: sphere.mana,
                });
        } else {
            let child = &mut self.castle_children[child_index];
            child.position = predicted;
            child.yaw = yaw as u16;
        }
        let child = &mut self.castle_children[child_index];
        child.control_flags = cave_control_flags;
        child.speed = cave_speed;
        child.cave_collision_cooldown = cave_collision_cooldown;
        if cave_sound {
            self.events
                .push(Level2EconomyEvent::CastleDefenderCaveSound {
                    defender_slot: source_slot,
                    sound_id: LEVEL2_CASTLE_DEFENDER_CAVE_SOUND,
                });
        }
        let child = self.castle_children[child_index];
        self.sync_castle_child_map(child);
        self.refresh_owner_mana_rows();
        true
    }

    /// Advance one live Level 2 carrier through its source target/return
    /// action.  The handler deliberately uses the imported Day terrain and
    /// economy-owned sphere list, so collection and Castle deposit mutate the
    /// same production state that the dispatcher hashes and exports.
    pub fn step_mana_carrier_slot(&mut self, carrier_slot: u16, terrain: &SessionTerrain) -> bool {
        let Some(original) = self
            .carriers
            .iter()
            .find(|carrier| carrier.active && carrier.source_entity_index == carrier_slot)
            .copied()
        else {
            return false;
        };
        let mut position = original.position;
        let mut yaw = original.yaw;
        let mut carried_mana = original.carried_mana;
        let mut owner_entity_index = original.owner_entity_index;
        let mut life = original.life;
        let mut target_sphere_id = original.target_sphere_id;
        let mut returning_to_castle = original.returning_to_castle;
        let mut valid_target = false;

        if original.returning_to_castle {
            if let Some(castle) = self.castle_for_owner(original.owner_entity_index).copied() {
                valid_target = true;
                yaw = i16::try_from(heading_for_positions(position, castle.position))
                    .expect("Level 2 carrier Castle heading fits");
                let arrival = u32::from(original.speed.unsigned_abs())
                    .saturating_mul(u32::from(castle.level));
                // AddBallon_60AB0 moves only while outside the Castle arrival
                // radius.  When the carrier is within that radius but fails
                // the altitude/stage gate, the source leaves its position
                // unchanged and lets the common terrain relaxation run.
                if level2_wrapping_xy_distance(position, castle.position)
                    <= arrival.saturating_mul(arrival)
                {
                    let ground = terrain.ground_height(position);
                    if position.z <= ground.wrapping_add(MANA_CARRIER_GROUND_CLEARANCE)
                        && castle.level > 0
                    {
                        position.x = castle.position.x;
                        position.y = castle.position.y;
                        let deposited = carried_mana;
                        self.castle_at_mut(castle.source_entity_index).expect("Castle remains installed").mana =
                            castle.mana.wrapping_add(deposited);
                        carried_mana = 0;
                        owner_entity_index = castle.owner_entity_index;
                        life = original.max_life;
                        target_sphere_id = 0;
                        returning_to_castle = false;
                        self.events.push(Level2EconomyEvent::ManaCarrierArrived {
                            carrier_id: original.id,
                            castle_id: castle.source_entity_index,
                            mana: deposited,
                        });
                    }
                } else {
                    crate::move_entity(&mut position, yaw, original.pitch, original.speed);
                }
            }
        } else if target_sphere_id != 0 {
            let sphere_index = self.spheres.iter().position(|sphere| {
                sphere.active && u32::from(sphere.source_entity_index) == target_sphere_id
            });
            if let Some(sphere_index) = sphere_index {
                valid_target = true;
                let sphere = self.spheres[sphere_index];
                yaw = i16::try_from(heading_for_positions(position, sphere.position))
                    .expect("Level 2 carrier sphere heading fits");
                if level2_carrier_sphere_relation_matches(&original, &sphere) {
                    let distance = level2_wrapping_xy_distance(position, sphere.position);
                    if distance
                        > u32::from(MANA_CARRIER_ATTRACTION_RADIUS)
                            .saturating_mul(u32::from(MANA_CARRIER_ATTRACTION_RADIUS))
                    {
                        self.spheres[sphere_index].assigned_carrier_id = 0;
                    } else {
                        self.spheres[sphere_index].assigned_carrier_id = u32::from(carrier_slot);
                        if level2_carrier_sphere_overlap(position, &sphere) {
                            carried_mana = carried_mana.wrapping_add(sphere.mana);
                            owner_entity_index = sphere.owner_entity_index;
                            life = original.max_life;
                            target_sphere_id = 0;
                            self.spheres[sphere_index].pending_delete = true;
                            self.spheres[sphere_index].control_flags |= ENTITY_DRAW_DISABLED_BIT;
                            self.events.push(Level2EconomyEvent::ManaSphereCollected {
                                sphere_id: sphere.id,
                                carrier_id: original.id,
                                mana: sphere.mana,
                            });
                        }
                    }
                    let speed_squared = u32::from(original.speed.unsigned_abs())
                        .saturating_mul(u32::from(original.speed.unsigned_abs()));
                    if distance <= speed_squared {
                        position.x = sphere.position.x;
                        position.y = sphere.position.y;
                    } else {
                        crate::move_entity(&mut position, yaw, original.pitch, original.speed);
                    }
                }
            }
        }

        if valid_target {
            let ground = terrain.ground_height(position);
            if position.z > ground {
                position.z = position.z.wrapping_add(MANA_CARRIER_DESCENT);
            }
            if position.z <= ground.wrapping_add(MANA_CARRIER_GROUND_CLEARANCE) {
                position.z = ground.wrapping_add(MANA_CARRIER_GROUND_CLEARANCE);
            }
        }

        let carrier = self
            .carriers
            .iter_mut()
            .find(|carrier| carrier.active && carrier.source_entity_index == carrier_slot)
            .expect("carrier remains installed");
        carrier.position = position;
        carrier.yaw = yaw;
        carrier.carried_mana = carried_mana;
        carrier.owner_entity_index = owner_entity_index;
        carrier.life = life;
        carrier.target_sphere_id = target_sphere_id;
        carrier.returning_to_castle = returning_to_castle;
        if carrier.life >= 0 && carrier.pending_attacker_entity_index != 0 {
            carrier.life = carrier.life.wrapping_sub(carrier.pending_damage);
            if carrier.life >= 0 {
                carrier.pending_attacker_entity_index = 0;
            }
        }
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: carrier_slot,
                active: carrier.active,
                x: position.x,
                y: position.y,
            })
            .expect("Level 2 carrier source slot fits the map chain");
        self.refresh_owner_mana_rows();
        true
    }

    /// Source `sub_36D50`: merge one active mana sphere into another and
    /// preserve owner arbitration. Neutral ownership adopts the other owner;
    /// class-10 owners yield to a non-class-10 owner; otherwise the source
    /// compares imported owner max-mana values (ties select the source side).
    pub fn merge_mana_spheres(&mut self, target_slot: u16, source_slot: u16) -> bool {
        if target_slot == source_slot {
            return false;
        }
        let Some(target_index) = self
            .spheres
            .iter()
            .position(|sphere| sphere.active && sphere.source_entity_index == target_slot)
        else {
            return false;
        };
        let Some(source_index) = self
            .spheres
            .iter()
            .position(|sphere| sphere.active && sphere.source_entity_index == source_slot)
        else {
            return false;
        };
        let target_owner = self.spheres[target_index].owner_entity_index;
        let source_owner = self.spheres[source_index].owner_entity_index;
        let target_relation = self.spheres[target_index].player_entity_index;
        let source_relation = self.spheres[source_index].player_entity_index;
        let source_control_flags = self.spheres[source_index].control_flags;
        // `sub_36D50` arbitrates through the sole source
        // `playerEntityIndex_0x94` relation.  The Rust semantic owner may
        // intentionally differ (for example after a Nyphur relation write),
        // so select a side by the raw relation and copy both projections from
        // that winning sphere instead of treating either field as an alias.
        let source_wins = if target_relation == 0 {
            source_relation != 0
        } else if source_relation == 0 || target_relation == source_relation {
            false
        } else if self.owner_class.get(&target_relation) == Some(&10)
            && self.owner_class.get(&source_relation) == Some(&10)
        {
            false
        } else if self.owner_class.get(&target_relation) == Some(&10)
            && self.owner_class.get(&source_relation) != Some(&10)
        {
            true
        } else if self.owner_class.get(&source_relation) == Some(&10)
            && self.owner_class.get(&target_relation) != Some(&10)
        {
            false
        } else if self
            .owner_max_mana
            .get(&target_relation)
            .copied()
            .unwrap_or_default()
            > self
                .owner_max_mana
                .get(&source_relation)
                .copied()
                .unwrap_or_default()
        {
            false
        } else {
            true
        };
        let owner = if source_wins {
            source_owner
        } else {
            target_owner
        };
        let player_relation = if source_wins {
            source_relation
        } else {
            target_relation
        };
        let source_mana = self.spheres[source_index].mana;
        let target_id = self.spheres[target_index].id;
        let source_id = self.spheres[source_index].id;
        let source_entity_rng = self.spheres[source_index].entity_rng;
        let source_position = self.spheres[source_index].position;
        self.spheres[target_index].mana = self.spheres[target_index].mana.wrapping_add(source_mana);
        self.spheres[target_index].owner_entity_index = owner;
        self.spheres[target_index].player_entity_index = player_relation;
        if target_relation == 0 && source_relation != 0 && source_control_flags & 0x0020_0000 != 0 {
            // Neutral-target adoption also inherits source byte-2 bit 0x20.
            self.spheres[target_index].control_flags |= 0x0020_0000;
        }
        self.spheres[source_index].assigned_carrier_id = 0;
        self.spheres[source_index].assigned_defender_slot = 0;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: false,
                x: source_position.x,
                y: source_position.y,
            })
            .expect("Level 2 merged sphere source slot fits the map chain");
        self.events.push(Level2EconomyEvent::ManaSpheresMerged {
            target_sphere_id: target_id,
            source_sphere_id: source_id,
            owner_entity_index: owner,
            mana: source_mana,
        });
        self.spheres.remove(source_index);
        self.release_slot(source_slot, Some(source_entity_rng));
        self.refresh_owner_mana_rows();
        true
    }

    /// Source carrier action 9's sphere absorption tail. The carrier receives
    /// the entire sphere payload only when the target is owned by the same
    /// player, within the Day attraction radius, and inside the strict source
    /// AABB (`AddBallon_60AB0`/`sub_106C0`). The sphere is disabled for the
    /// next reclaim pass; no allocation is attempted on this path.
    pub fn absorb_sphere_into_carrier(&mut self, sphere_slot: u16, carrier_slot: u16) -> bool {
        let Some(carrier) = self
            .carriers
            .iter()
            .copied()
            .find(|carrier| carrier.active && carrier.source_entity_index == carrier_slot)
        else {
            return false;
        };
        let Some(sphere) = self
            .spheres
            .iter()
            .copied()
            .find(|sphere| sphere.active && sphere.source_entity_index == sphere_slot)
        else {
            return false;
        };
        if !level2_carrier_sphere_relation_matches(&carrier, &sphere)
            || level2_wrapping_xy_distance(carrier.position, sphere.position)
                > u32::from(MANA_CARRIER_ATTRACTION_RADIUS)
                    * u32::from(MANA_CARRIER_ATTRACTION_RADIUS)
            || !level2_carrier_sphere_overlap(carrier.position, &sphere)
        {
            return false;
        }
        let mana = sphere.mana;
        let carrier_state = self
            .carriers
            .iter_mut()
            .find(|carrier| carrier.active && carrier.source_entity_index == carrier_slot)
            .expect("carrier remains installed");
        carrier_state.carried_mana = carrier_state.carried_mana.wrapping_add(mana);
        carrier_state.target_sphere_id = 0;
        carrier_state.owner_entity_index = sphere.owner_entity_index;
        carrier_state.life = carrier_state.max_life;
        let sphere_state = self
            .spheres
            .iter_mut()
            .find(|candidate| candidate.active && candidate.source_entity_index == sphere_slot)
            .expect("sphere remains installed");
        sphere_state.assigned_carrier_id = 0;
        sphere_state.assigned_defender_slot = 0;
        sphere_state.pending_delete = true;
        sphere_state.control_flags |= ENTITY_DRAW_DISABLED_BIT;
        self.events.push(Level2EconomyEvent::ManaSphereCollected {
            sphere_id: sphere.id,
            carrier_id: carrier.id,
            mana,
        });
        self.refresh_owner_mana_rows();
        true
    }

    pub fn spawn_mana_carrier(
        &mut self,
        position: Position,
        owner_entity_index: u16,
        target_sphere_id: u32,
    ) -> Option<u16> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let carrier = ManaCarrier {
            id: u32::from(source_entity_index),
            source_entity_index,
            entity_rng,
            owner_entity_index,
            position,
            yaw: 0,
            pitch: 0,
            speed: MANA_CARRIER_SPEED,
            life: MANA_CARRIER_MAX_LIFE,
            max_life: MANA_CARRIER_MAX_LIFE,
            max_mana: MANA_CARRIER_MAX_MANA,
            target_sphere_id,
            carried_mana: 0,
            returning_to_castle: false,
            pending_damage: 0,
            pending_attacker_entity_index: 0,
            active: true,
        };
        self.source_phase_sidecars
            .insert(source_entity_index, source_entity_index as u8);
        self.carriers.push(carrier);
        self.refresh_owner_mana_rows();
        self.events.push(Level2EconomyEvent::ManaCarrierSpawned {
            source_entity_index,
            owner_entity_index,
        });
        Some(source_entity_index)
    }

    pub fn build_castle(
        &mut self,
        position: Position,
        owner_entity_index: u16,
        terrain: &SessionTerrain,
    ) -> Option<u16> {
        let mut root_position = position;
        root_position.z = terrain.ground_height(root_position);
        self.build_castle_at(
            root_position,
            root_position,
            owner_entity_index,
            false,
            true,
        )
    }

    /// Build a Castle from the source projectile placement path, retaining
    /// the snapped root position and BUILD-derived construction anchor. This
    /// is the production entry point when the owner BUILD
    /// footprint is available from the validated cache.
    pub fn build_castle_with_footprint(
        &mut self,
        projectile_position: Position,
        owner_entity_index: u16,
        footprint: &CastleBuildFootprint,
        terrain: &SessionTerrain,
    ) -> Option<u16> {
        self.build_castle_with_footprint_mode(
            projectile_position,
            owner_entity_index,
            footprint,
            true,
            terrain,
        )
    }

    fn build_castle_with_footprint_mode(
        &mut self,
        projectile_position: Position,
        owner_entity_index: u16,
        footprint: &CastleBuildFootprint,
        emit_built_event: bool,
        terrain: &SessionTerrain,
    ) -> Option<u16> {
        if footprint.index != 1 {
            return None;
        }
        let (root_position, construction_anchor) =
            level2_castle_root_positions(terrain, footprint, projectile_position);
        let castle_slot = self.build_castle_at(
            root_position,
            construction_anchor,
            owner_entity_index,
            true,
            emit_built_event,
        );
        if castle_slot.is_some() {
            self.castle_footprint = Some(footprint.clone());
            self.castle_stage_footprints
                .insert(footprint.index, footprint.clone());
        }
        castle_slot
    }

    /// Start the ordinary human `sub_69AB0` Castle projectile path with the
    /// source-derived launch payload. The class-9/model-10 flyer is a
    /// distinct source row; collision allocates the later class-3/model-2
    /// Castle seed and leaves the player relation untouched until that
    /// seed's construction child succeeds.
    pub fn spawn_castle_projectile_with_payload(
        &mut self,
        payload: Level2CastleProjectileLaunchPayload,
        holder_entity_index: u16,
        player_actual_speed: i16,
        terrain: &SessionTerrain,
    ) -> Option<u16> {
        let footprint = self.castle_stage_footprints.get(&1)?.clone();
        let target_castle_slot = self
            .castle_for_owner(payload.source_id_entity_index)
            .map_or(0, |castle| castle.source_entity_index);
        let axis_position = if target_castle_slot == 0 {
            payload.axis_position
        } else {
            self.castle_at(target_castle_slot)
                .map_or(payload.position, |castle| castle.position)
        };
        let (target_position, target_anchor) =
            level2_castle_root_positions(terrain, &footprint, axis_position);
        let initial_yaw = payload.yaw;
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let projectile = Level2CastleProjectile {
            source_entity_index,
            entity_rng,
            // `owner_entity_index` is the existing hashed carrier slot for
            // the source `id_0x1A` relation. The dedicated subspell/mana/A4
            // fields are carried by the payload so the owning world module
            // can add their lossless carrier projection without a parallel
            // side table.
            owner_entity_index: payload.source_id_entity_index,
            holder_entity_index,
            position: payload.position,
            axis_position,
            target_position,
            target_anchor,
            source_class: 9,
            source_model: 10,
            source_byte_43: if target_castle_slot == 0 { 3 } else { 10 },
            source_byte_44: if target_castle_slot == 0 { 2 } else { 43 },
            source_word_96: target_castle_slot,
            source_subspell_index: payload.source_subspell_index,
            source_mana: payload.source_mana,
            source_a4_byte_154: payload.source_a4_byte_154,
            target_castle_slot,
            yaw: initial_yaw,
            pitch: payload.pitch,
            actual_speed: 384_i16.wrapping_add(player_actual_speed),
            minimum_speed: 384,
            life: 0x2_000 / 384,
            max_life: 0x2_000 / 384,
            control_flags: 0,
            site_rejected: false,
            active: true,
            drawing_disabled: false,
        };
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: payload.position.x,
                y: payload.position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, Some(entity_rng));
            return None;
        }
        self.castle_projectiles.push(projectile);
        self.source_phase_sidecars
            .insert(source_entity_index, source_entity_index as u8);
        self.source_owner
            .insert(source_entity_index, payload.source_id_entity_index);
        Some(source_entity_index)
    }

    /// Compatibility wrapper for direct economy tests and the pre-payload
    /// caller. Production code should pass the source-shaped payload above;
    /// this wrapper deliberately supplies only the old raw-position/zero-
    /// pitch defaults.
    pub fn spawn_castle_projectile(
        &mut self,
        projectile_position: Position,
        owner_entity_index: u16,
        holder_entity_index: u16,
        player_yaw: i16,
        player_next_entity: i16,
        player_actual_speed: i16,
        terrain: &SessionTerrain,
    ) -> Option<u16> {
        self.spawn_castle_projectile_with_payload(
            Level2CastleProjectileLaunchPayload {
                position: projectile_position,
                axis_position: {
                    let mut target = projectile_position;
                    crate::move_entity(
                        &mut target,
                        player_next_entity.wrapping_add(player_yaw),
                        0,
                        4_096,
                    );
                    target.z = terrain.ground_height(target);
                    target
                },
                yaw: player_next_entity.wrapping_add(player_yaw),
                pitch: 0,
                source_id_entity_index: owner_entity_index,
                source_subspell_index: 0,
                source_mana: 0,
                source_a4_byte_154: 0,
            },
            holder_entity_index,
            player_actual_speed,
            terrain,
        )
    }

    /// Advance one class-9/model-10 Castle flyer. A new-Castle flyer starts
    /// with the source gate-only sub_66D00 visit; an occupied flyer enters
    /// CastCastleProjectile_66B30 immediately. Later callbacks use the
    /// source-shaped transport and land only on the source collision/terrain/
    /// life predicates. Allocation failure leaves the flyer live so the
    /// source collision callback can retry.
    pub fn step_castle_projectile_slot(
        &mut self,
        source_slot: u16,
        entities: &[EntitySnapshot],
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(projectile) = self
            .castle_projectiles
            .iter()
            .find(|projectile| projectile.active && projectile.source_entity_index == source_slot)
            .copied()
        else {
            return false;
        };
        if projectile.source_word_96 == 0
            && projectile.control_flags & LEVEL2_CASTLE_CONSTRUCTION_INITIALIZED_BIT == 0
        {
            if self.castle_projectile_site_blocked(projectile.position, entities, terrain) {
                self.disable_castle_projectile(source_slot, true);
                return false;
            }
            if let Some(projectile) = self.castle_projectiles.iter_mut().find(|projectile| {
                projectile.active && projectile.source_entity_index == source_slot
            }) {
                projectile.control_flags |= LEVEL2_CASTLE_CONSTRUCTION_INITIALIZED_BIT;
                projectile.life = projectile.max_life;
            }
            return true;
        }
        let target = if projectile.source_word_96 != 0 {
            self.castle_at(projectile.source_word_96)
                .map_or(projectile.axis_position, |castle| castle.position)
        } else {
            projectile.axis_position
        };
        let (landed, _gate_blocked, landing_position) = {
            let mut moved = projectile;
            let distance = level2_castle_projectile_distance(moved.position, target);
            if distance <= i32::from(moved.actual_speed) {
                moved.position = target;
            } else {
                level2_move_castle_projectile_toward(&mut moved, target);
            }
            let castle_hit = self.castle_at(projectile.source_word_96).filter(|castle| {
                projectile.source_word_96 != 0
                    && level2_model76_sub106c0_matches(
                        moved.position,
                        0,
                        0,
                        0,
                        0,
                        castle.position,
                        castle.shift_yaw,
                        castle.shift_pitch,
                        castle.shift_roll,
                        castle.shift_fov,
                    )
            });
            if castle_hit.is_some() {
                if let Some(castle) = castle_hit {
                    moved.position = castle.position;
                }
            }
            let mut terrain_hit = false;
            if castle_hit.is_none() {
                let ground = terrain.ground_height(moved.position);
                let moved_z = moved.position.z;
                if ground > moved_z {
                    moved.position.z = ground;
                    terrain_hit = true;
                }
                let moved_z = moved.position.z;
                if terrain.map_type == MapType::Cave
                    && let Some(ceiling) = terrain.ceiling_height(moved.position)
                    && moved_z > ceiling
                {
                    moved.position.z = ceiling;
                    terrain_hit = true;
                }
            }
            moved.life = moved.life.wrapping_sub(1);
            let expired = moved.life < 0;
            let collision_landed = castle_hit.is_some() || terrain_hit || expired;
            // `sub_66D00` re-runs `sub_11CB0` after movement.  A failed
            // later gate is reached only after the castle-hit, terrain, and
            // life branches have all failed.  It is then itself the source
            // landing predicate: rotate yaw by 0x400 and perform one
            // same-speed bounce before allocation.
            let gate_blocked = !collision_landed
                && projectile.source_word_96 == 0
                && self.castle_projectile_site_blocked(moved.position, entities, terrain);
            let landed = collision_landed || gate_blocked;
            if gate_blocked {
                moved.yaw = (moved.yaw.wrapping_add(0x400)) & 0x07ff;
                crate::move_entity(
                    &mut moved.position,
                    moved.yaw,
                    moved.pitch,
                    moved.actual_speed,
                );
            }
            if let Some(live) = self
                .castle_projectiles
                .iter_mut()
                .find(|live| live.active && live.source_entity_index == source_slot)
            {
                *live = moved;
                let _ = self.map_chains.sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: true,
                    x: moved.position.x,
                    y: moved.position.y,
                });
            }
            (landed, gate_blocked, moved.position)
        };
        if !landed {
            return true;
        }
        if projectile.source_byte_43 == 3 && projectile.source_byte_44 == 2 {
            let footprint = self.castle_stage_footprints.get(&1).cloned();
            let Some(footprint) = footprint else {
                return true;
            };
            let (root_position, construction_anchor) =
                level2_castle_root_positions(terrain, &footprint, landing_position);
            if self
                .allocate_castle_root_seed(
                    root_position,
                    construction_anchor,
                    projectile.owner_entity_index,
                )
                .is_none()
            {
                return true;
            }
        } else if self
            .allocate_castle_upgrade_product(landing_position, projectile.source_word_96)
            .is_none()
        {
            return true;
        }
        self.disable_castle_projectile(source_slot, false);
        false
    }

    fn allocate_castle_upgrade_product(
        &mut self,
        position: Position,
        target_castle_slot: u16,
    ) -> Option<u16> {
        let root = *self.castle_at(target_castle_slot)?;
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let product = Level2CastleChild {
            source_entity_index,
            entity_rng,
            id_entity_index: root.source_id_entity_index,
            source_class: 10,
            parent_entity_index: root.source_entity_index,
            owner_entity_index: root.owner_entity_index,
            player_entity_index: 0,
            position,
            axis_position: Position::default(),
            model: 43,
            action: 45,
            mana: 0,
            max_mana: 0,
            life: 8,
            max_life: 8,
            speed: 0,
            target_sphere_slot: 0,
            pending_damage: 0,
            pending_attacker_entity_index: 0,
            damage_attacker_entity_index: 0,
            killer_entity_index: 0,
            source_byte_3e: 0,
            source_byte_39: 0,
            source_byte_3b: 0,
            cave_collision_cooldown: 0,
            source_dword_10: 0,
            source_word_2e: 0,
            construction_target_height: 0,
            source_subspell_index: 64_000,
            yaw: 512,
            roll: 512,
            visual_index: 0,
            control_flags: 1,
            chain_previous_slot: 0,
            chain_next_slot: 0,
            source_byte_43: 0,
            source_word_4a: 0,
            active: true,
        };
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, Some(entity_rng));
            return None;
        }
        self.castle_children.push(product);
        Some(source_entity_index)
    }

    /// Source `sub_11CB0`/`sub_11C80`'s first flyer-tick placement gate for
    /// the non-cave Day route. It scans the signed map-angle flags in the
    /// fixed 8x8 window around the flyer, then rejects a nearby owned
    /// class-3/model-2 Castle row. BUILD cell payloads are construction data,
    /// not placement validity.
    pub(crate) fn castle_projectile_site_blocked(
        &self,
        position: Position,
        entities: &[EntitySnapshot],
        terrain: &SessionTerrain,
    ) -> bool {
        let start_x = (position.x >> 8) as u8;
        let start_y = (position.y >> 8) as u8;
        let nearby_castle = self.live_castles().any(|castle| {
            castle.source_class == 3
                && castle.source_model == 2
                && level2_castle_nearby_projectile(*castle, position)
        }) || entities.iter().any(|entity| {
            entity.active
                && entity.life >= 0
                && entity.class == 3
                && entity.model == 2
                && level2_entity_nearby_projectile(entity, position)
        });
        let nearby_model45 = entities.iter().any(|entity| {
            entity.active
                && entity.class == 10
                && entity.model == 45
                && level2_entity_nearby_projectile(entity, position)
        });
        if nearby_castle || nearby_model45 {
            return true;
        }
        for row in 0..8_u8 {
            for column in 0..8_u8 {
                let flags = terrain.angle_flags(crate::Tile {
                    x: start_x.wrapping_sub(8).wrapping_add(column),
                    y: start_y.wrapping_sub(8).wrapping_add(row),
                });
                if flags.cast_signed() < 0 {
                    return true;
                }
            }
        }
        false
    }

    fn disable_castle_projectile(&mut self, source_slot: u16, site_rejected: bool) {
        let Some(projectile) = self
            .castle_projectiles
            .iter_mut()
            .find(|projectile| projectile.active && projectile.source_entity_index == source_slot)
        else {
            return;
        };
        // DisableEntityDrawing hides the source row but does not call
        // SetMapEntity. The row remains map/arena-reserved until the next
        // UpdateEntities hidden-row prepass reclaims it.
        projectile.active = false;
        projectile.drawing_disabled = true;
        projectile.site_rejected = site_rejected;
    }

    #[must_use]
    pub fn rejected_castle_projectile_holder(&self, source_slot: u16) -> Option<u16> {
        self.castle_projectiles
            .iter()
            .find(|projectile| {
                projectile.source_entity_index == source_slot
                    && projectile.site_rejected
                    && !projectile.active
            })
            .map(|projectile| projectile.holder_entity_index)
    }

    pub fn take_castle_upgrade_holder_clear_pending(&mut self) -> bool {
        let local_castle_slot = self.player_castle_entity_index();
        let pending = self.castle_at(local_castle_slot)
            .is_some_and(|castle| castle.upgrade_holder_clear_pending);
        if let Some(castle) = self.castle_at_mut(local_castle_slot) {
            castle.upgrade_holder_clear_pending = false;
        }
        pending
    }

    /// Bind one preserved DATA_BUILD00DATTAB record for later
    /// `RemoveCastleStage_385C0` calls.  The stage byte is intentionally
    /// separate from the stage-one construction convenience field.
    pub fn set_castle_stage_footprint(
        &mut self,
        stage: u8,
        footprint: CastleBuildFootprint,
    ) -> bool {
        let (expected_width, expected_height) = level2_castle_removal_dimensions(stage);
        if !(1..=7).contains(&stage)
            || footprint.index != stage
            || (footprint.width, footprint.height) != (expected_width, expected_height)
        {
            return false;
        }
        self.castle_stage_footprints.insert(stage, footprint);
        true
    }

    #[must_use]
    pub(crate) fn has_castle_stage_footprint(&self, stage: u8) -> bool {
        self.castle_stage_footprints.contains_key(&stage)
    }

    fn install_castle_root(
        &mut self,
        source_entity_index: u16,
        position: Position,
        anchor_position: Position,
        owner_entity_index: u16,
        source_constructor: bool,
        emit_built_event: bool,
        sync_map: bool,
    ) -> Option<u16> {
        let slot_generation = self.slot_generation(source_entity_index);
        let owner_slot_generation = self.slot_generation(owner_entity_index);
        self.castle.0.push(Level2CastleState {
            source_entity_index,
            slot_generation,
            owner_slot_generation,
            // The source Castle root stores the owning player's logical
            // entity id in id_0x1A; the arena slot remains separate.
            source_id_entity_index: owner_entity_index,
            player_entity_index: if source_constructor { 0 } else { owner_entity_index },
            source_class: 3,
            source_model: 2,
            source_byte_38: 33,
            source_particle_parameter: 177,
            owner_entity_index,
            position,
            anchor_position,
            life: if source_constructor {
                40_000
            } else {
                CASTLE_STAGE_ONE_LIFE
            },
            max_life: if source_constructor {
                40_000
            } else {
                CASTLE_STAGE_ONE_LIFE
            },
            mana: 0,
            mana_capacity: if source_constructor {
                0
            } else {
                CASTLE_STAGE_ONE_MANA_CAPACITY
            },
            pending_damage: 0,
            pending_attacker_entity_index: 0,
            killer_entity_index: 0,
            control_flags: 0,
            timer: 0,
            phase: source_entity_index as u8,
            action: if source_constructor { 5 } else { 4 },
            build_state: if source_constructor { 0 } else { 4 },
            level: if source_constructor { 0 } else { 1 },
            construction_slot: 0,
            defense_slot: 0,
            construction_model: 0,
            construction_action: 0,
            construction_control_flags: 0,
            construction_parent_slot: 0,
            construction_owner_entity_index: 0,
            construction_id: 0,
            construction_position: Position::default(),
            defense_model: 0,
            defense_action: 0,
            defense_control_flags: 0,
            defense_parent_slot: 0,
            defense_owner_entity_index: 0,
            defense_id: 0,
            defense_position: Position::default(),
            child_chain_slots: [0; 2],
            child_chain_models: [0; 2],
            child_chain_active: [false; 2],
            controller_chain_slots: [0; 8],
            controller_chain_count: 0,
            controller_chain_head: 0,
            controller_chain_tail: 0,
            shift_pitch: level2_castle_shift_axes(if source_constructor { 0 } else { 1 }).0,
            shift_roll: level2_castle_shift_axes(if source_constructor { 0 } else { 1 }).1,
            shift_yaw: 0,
            shift_fov: 256,
            source_word_2e: 0,
            source_word_30: 0,
            source_word_7c: 0,
            source_word_80: 0,
            upgrade_holder_clear_pending: false,
            defender_slots: [0; 3],
            defender_count: 0,
            defender_total_mana: 0,
            defender_total_max_mana: 0,
            defender_target_spheres: [0; 3],
            auxiliary_slots: [0; 34],
            auxiliary_count: 0,
            auxiliary_cooldown: 0,
            auxiliary_positions: [Position::default(); 34],
            auxiliary_yaws: [0; 34],
            auxiliary_rolls: [0; 34],
            auxiliary_player_entity_indices: [0; 34],
            construction_child_active: false,
            defense_child_active: false,
            active: true,
        });
        if self.local_player_entity_index == 0 {
            self.local_player_entity_index = owner_entity_index;
        }
        if !source_constructor {
            self.link_castle_owner(owner_entity_index, source_entity_index);
        }
        if owner_entity_index == self.local_player_entity_index {
            self.player_castle_entity_index = if source_constructor { 0 } else { source_entity_index };
            self.player_castle_stage = if source_constructor { 0 } else { 1 };
            self.player_castle_aux_flags = 0;
        }
        if sync_map {
            self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .expect("Castle root source slot fits the map chain");
        }
        if emit_built_event {
            self.events.push(Level2EconomyEvent::CastleBuilt {
                source_entity_index,
                owner_entity_index,
            });
        }
        Some(source_entity_index)
    }

    /// Transfer the source reset's Castle roots and in-flight construction
    /// children into the same arena used by newly cast Castles. Unsupported
    /// source shapes remain in the imported arena; they are not silently
    /// replaced by an invented empty defender/auxiliary list.
    pub(crate) fn adopt_imported_castles(
        &mut self,
        players: &[PlayerSnapshot],
        entities: &mut [EntitySnapshot],
    ) -> usize {
        let word = |row: &EntitySnapshot, at: usize| {
            u16::from_le_bytes([row.record[at], row.record[at + 1]])
        };
        // MCE1 retains a four-byte slot/active header before the source
        // entity. Castle state and timer are source words 0x2e and 0x30.
        let castle_state = |row: &EntitySnapshot| word(row, 4 + 0x2e);
        let castle_timer = |row: &EntitySnapshot| word(row, 4 + 0x30);
        let dword = |row: &EntitySnapshot, at: usize| {
            i32::from_le_bytes(row.record[at..at + 4].try_into().expect("source record word"))
        };
        let axis = |row: &EntitySnapshot| Position {
            x: word(row, 158),
            y: word(row, 160),
            z: word(row, 162) as i16,
        };
        let mut candidates = Vec::new();
        for player in players {
            let relation = i16::from_le_bytes([player.record[42], player.record[43]]);
            let Ok(slot) = u16::try_from(relation) else { continue };
            if slot == 0 || player.entity_index == 0 || self.castle_for_owner(player.entity_index).is_some() {
                continue;
            }
            let Some(root) = entities.get(usize::from(slot)).filter(|row| {
                row.active && row.class == 3 && row.model == 2 && row.action == 5
                    && row.source_id == player.entity_index && matches!(castle_state(row), 2 | 4)
                    && (1..=7).contains(&row.source_dword_10())
            }) else { continue };
            // These owner arrays are not present in the compact export. Do
            // not discard live rows whose source relation cannot be restored.
            if entities.iter().any(|row| row.active
                && ((row.class == 3 && row.model == 3) || (row.class == 5 && row.model == 15) || (row.class == 10 && row.model == 79))
                && (row.source_id == player.entity_index || row.source_player_entity_index() == player.entity_index)) {
                continue;
            }
            let children = entities.iter().filter(|row| row.active && word(row, 44) == slot).collect::<Vec<_>>();
            let child = match (castle_state(root), children.as_slice()) {
                (2, []) => None,
                (4, [row]) if row.class == 10 && row.model == 42 && row.action == 44
                    && row.source_id == player.entity_index => Some((*row).clone()),
                _ => continue,
            };
            candidates.push((player.entity_index, root.clone(), child));
        }
        let mut adopted = 0;
        for (owner, root, child) in candidates {
            let slot = root.index;
            // A reset row starts at generation zero but is now owned by the
            // typed arena. Recording that generation prevents the baseline
            // mana row from surviving after this root is destroyed.
            self.slot_generations.entry(slot).or_default();
            let _ = self.install_castle_root(slot, root.position, axis(&root), owner, true, false, false);
            self.source_rng.insert(slot, root.entity_rng);
            self.source_owner.insert(slot, owner);
            let castle = self.castle_entry_mut(slot).expect("imported Castle installed");
            castle.source_id_entity_index = root.source_id;
            castle.player_entity_index = root.source_player_entity_index();
            castle.source_byte_38 = root.record[60];
            castle.source_particle_parameter = word(&root, 94);
            castle.life = root.life;
            castle.max_life = root.max_life;
            castle.mana = root.mana;
            castle.mana_capacity = root.max_mana;
            castle.pending_damage = dword(&root, 98);
            castle.pending_attacker_entity_index = word(&root, 102);
            castle.killer_entity_index = word(&root, 40);
            castle.control_flags = root.control_flags;
            castle.phase = root.phase;
            castle.action = root.action;
            castle.build_state = castle_state(&root) as u8;
            castle.level = root.source_dword_10() as u8;
            castle.shift_yaw = root.array_yaw();
            castle.shift_pitch = root.array_pitch();
            castle.shift_roll = root.array_roll();
            castle.shift_fov = root.array_fov();
            castle.source_word_2e = castle_state(&root);
            castle.source_word_30 = castle_timer(&root);
            castle.timer = castle_timer(&root) as u8;
            castle.source_word_7c = word(&root, 128);
            castle.source_word_80 = word(&root, 132);
            if let Some(row) = child {
                let child_slot = row.index;
                castle.construction_slot = child_slot;
                castle.construction_model = row.model;
                castle.construction_action = row.action;
                castle.construction_control_flags = row.control_flags;
                castle.construction_parent_slot = slot;
                castle.construction_owner_entity_index = owner;
                castle.construction_id = row.source_id;
                castle.construction_position = row.position;
                castle.child_chain_slots[0] = child_slot;
                castle.child_chain_models[0] = row.model;
                castle.child_chain_active[0] = true;
                castle.construction_child_active = true;
                self.slot_generations.entry(child_slot).or_default();
                self.castle_children.push(Level2CastleChild {
                    source_entity_index: child_slot, entity_rng: row.entity_rng,
                    id_entity_index: row.source_id, source_class: row.class,
                    parent_entity_index: slot, owner_entity_index: owner,
                    player_entity_index: row.source_player_entity_index(), position: row.position,
                    axis_position: axis(&row), model: row.model, action: row.action,
                    mana: row.mana, max_mana: row.max_mana, life: row.life, max_life: row.max_life,
                    speed: row.actual_speed, target_sphere_slot: row.source_word_96(),
                    pending_damage: dword(&row, 98), pending_attacker_entity_index: word(&row, 102),
                    damage_attacker_entity_index: word(&row, 42), killer_entity_index: word(&row, 40),
                    source_byte_3e: row.phase, source_byte_39: row.record[61], source_byte_3b: row.byte_3b,
                    cave_collision_cooldown: row.record[74], source_dword_10: row.source_dword_10(),
                    source_word_2e: castle_state(&row) as i16,
                    construction_target_height: row.subspell_index as i16, source_subspell_index: row.subspell_index,
                    yaw: row.yaw as u16, roll: row.source_roll() as u16, visual_index: word(&row, 94),
                    control_flags: row.control_flags, chain_previous_slot: word(&row, 54),
                    chain_next_slot: word(&row, 56), source_byte_43: row.record[71],
                    source_word_4a: word(&row, 78), active: true,
                });
                self.source_rng.insert(child_slot, row.entity_rng);
                entities[usize::from(child_slot)].active = false;
                entities[usize::from(child_slot)].record[2] = 0;
            }
            self.link_castle_owner(owner, slot);
            if owner == self.local_player_entity_index {
                self.player_castle_entity_index = slot;
                self.player_castle_stage = root.source_dword_10() as u8;
            }
            entities[usize::from(slot)].active = false;
            entities[usize::from(slot)].record[2] = 0;
            adopted += 1;
        }
        adopted
    }

    /// Allocate the class-3/model-2 row created by the human flyer collision
    /// callback.  This is deliberately separate from the authored/test
    /// `build_castle_at` path: the public spell route first owns a live
    /// class-9/model-10 flyer and only then creates this seed.
    fn allocate_castle_root_seed(
        &mut self,
        position: Position,
        anchor_position: Position,
        owner_entity_index: u16,
    ) -> Option<u16> {
        if self.live_castles().any(|castle| castle.owner_entity_index == owner_entity_index) {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        // Preserve the root's live `rand_0x14` separately from the reclaim
        // stack seed, matching NewEvent's source-owned RNG stream.
        let root_source_rng = self
            .source_rng
            .get(&source_entity_index)
            .copied()
            .unwrap_or_else(|| self.new_event_rng(source_entity_index));
        self.source_rng.insert(source_entity_index, root_source_rng);
        self.install_castle_root(
            source_entity_index,
            position,
            anchor_position,
            owner_entity_index,
            true,
            false,
            true,
        )
    }

    pub(super) fn build_castle_at(
        &mut self,
        position: Position,
        anchor_position: Position,
        owner_entity_index: u16,
        source_constructor: bool,
        emit_built_event: bool,
    ) -> Option<u16> {
        if self.live_castles().any(|castle| castle.owner_entity_index == owner_entity_index) {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        // Preserve the root's live `rand_0x14` separately from the reclaim
        // stack seed.  Overflow and the stage-removal sentinel both advance
        // this stream, so falling back to `slot_rng` after the first call
        // would replay the wrong source sequence.
        let root_source_rng = self
            .source_rng
            .get(&source_entity_index)
            .copied()
            .unwrap_or_else(|| self.new_event_rng(source_entity_index));
        self.source_rng.insert(source_entity_index, root_source_rng);
        self.install_castle_root(
            source_entity_index,
            position,
            anchor_position,
            owner_entity_index,
            source_constructor,
            emit_built_event,
            true,
        )
    }

    /// Set the source Castle anchor (`axis_0x9A_154x`) used by child creation
    /// and the action-4 model-39 chain scan. The root position remains the
    /// terrain-synchronised entity position.
    pub fn set_castle_anchor(&mut self, castle_slot: u16, anchor: Position) -> bool {
        let Some(castle) = self.castle_at_mut(castle_slot) else {
            return false;
        };
        castle.anchor_position = anchor;
        true
    }

    /// Re-enter the source Castle construction action for a reset root. This
    /// is the bounded `BeginOfCastleCreation_5FA70` entry; child allocation is
    /// committed only by the following `step_castle_action` call.
    pub fn start_castle_construction(&mut self, castle_slot: u16) -> bool {
        let Some(castle) = self.castle_at_mut(castle_slot) else {
            return false;
        };
        if castle.action != 4 {
            return false;
        }
        castle.action = 5;
        castle.build_state = 0;
        castle.level = 0;
        castle.construction_slot = 0;
        castle.defense_slot = 0;
        castle.timer = 0;
        castle.construction_model = 0;
        castle.construction_action = 0;
        castle.construction_control_flags = 0;
        castle.construction_parent_slot = 0;
        castle.construction_owner_entity_index = 0;
        castle.construction_id = 0;
        castle.construction_position = Position::default();
        castle.defense_model = 0;
        castle.defense_action = 0;
        castle.defense_control_flags = 0;
        castle.defense_parent_slot = 0;
        castle.defense_owner_entity_index = 0;
        castle.defense_id = 0;
        castle.defense_position = Position::default();
        castle.child_chain_slots = [0; 2];
        castle.child_chain_models = [0; 2];
        castle.child_chain_active = [false; 2];
        castle.controller_chain_slots = [0; 8];
        castle.controller_chain_count = 0;
        castle.controller_chain_head = 0;
        castle.controller_chain_tail = 0;
        castle.source_word_2e = 0;
        castle.source_word_30 = 0;
        castle.defender_slots = [0; 3];
        castle.defender_count = 0;
        castle.defender_total_mana = 0;
        castle.defender_total_max_mana = 0;
        castle.defender_target_spheres = [0; 3];
        castle.auxiliary_slots = [0; 34];
        castle.auxiliary_count = 0;
        castle.auxiliary_cooldown = 0;
        castle.auxiliary_positions = [Position::default(); 34];
        castle.auxiliary_yaws = [0; 34];
        castle.auxiliary_rolls = [0; 34];
        castle.auxiliary_player_entity_indices = [0; 34];
        castle.construction_child_active = false;
        castle.defense_child_active = false;
        castle.life = 0;
        castle.max_life = 0;
        castle.mana_capacity = 5_000;
        if self.player_castle_entity_index == castle_slot {
            self.player_castle_stage = 0;
            self.player_castle_aux_flags = 0;
        }
        true
    }

    /// Commit the model-42/37BC0 construction callback's `state 2` transition.
    /// The BUILD terrain completion is applied by the canonical terrain
    /// authority.  The source keeps the child allocated, disables only its
    /// drawing, and lets the following root tick consume state 2.
    pub fn complete_castle_construction(
        &mut self,
        castle_slot: u16,
        terrain: &mut SessionTerrain,
    ) -> bool {
        let Some(castle) = self.castle_at(castle_slot).copied().filter(|castle| {
            castle.action == 5
                && castle.build_state == 4
                && castle.construction_child_active
        }) else {
            return false;
        };
        let Some(child) = self
            .castle_children
            .iter()
            .find(|child| child.active && child.source_entity_index == castle.construction_slot)
            .copied()
        else {
            return false;
        };
        let Some(footprint) = self
            .castle_stage_footprints
            .get(&child.cave_collision_cooldown)
            .cloned()
        else {
            return false;
        };
        if let Some(castle) = self.castle_entry_mut(castle_slot) {
            castle.construction_child_active = false;
            castle.child_chain_active[0] = false;
            castle.build_state = 2;
            castle.source_word_2e = 2;
        }
        terrain
            .apply_castle_update(
                CastleTerrainUpdate::ConstructionCompleted {
                    position: child.position,
                },
                &footprint,
            )
            .expect("validated Castle BUILD footprint accepts completion");
        if let Some(child) = self
            .castle_children
            .iter_mut()
            .find(|candidate| candidate.source_entity_index == child.source_entity_index)
        {
            child.control_flags |= ENTITY_DRAW_DISABLED_BIT;
        }
        true
    }

    /// Enter the source model-41 defense-child state (`state 5`).
    pub fn start_castle_defense_child(&mut self, castle_slot: u16) -> bool {
        let Some(castle) = self.castle_at_mut(castle_slot) else {
            return false;
        };
        if castle.action != 5 || castle.build_state != 4 {
            return false;
        }
        castle.build_state = 5;
        true
    }

    fn advance_castle_phase(&mut self, castle_slot: u16) {
        if let Some(castle) = self.castle_entry_mut(castle_slot) {
            castle.phase = castle.phase.wrapping_add(1);
        }
    }

    pub(super) fn sync_castle_child_map(&mut self, child: Level2CastleChild) {
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: child.source_entity_index,
                active: child.active,
                x: child.position.x,
                y: child.position.y,
            })
            .expect("Castle child source slot fits the map chain");
    }

    /// Retire the source `word_0x34_52` controller chain by following each
    /// model-79 child's `word_0x34_52` successor.  `sub_613D0` does not use
    /// `parentId` as a release set: defenders, model-15 auxiliaries, and any
    /// other source-owned rows remain for the following `sub_5FF50` walk.
    fn retire_castle_controller_chain(&mut self, castle_slot: u16) {
        let mut current_slot = self
            .castle_entry(castle_slot)
            .map_or(0, |castle| castle.controller_chain_head);
        let mut visited = BTreeSet::new();
        let mut retired_slots = BTreeSet::new();
        while current_slot != 0 && visited.insert(current_slot) {
            let Some(child) = self
                .castle_children
                .iter()
                .find(|child| {
                    child.source_entity_index == current_slot
                        && child.source_class == 10
                        && child.model == 79
                })
                .copied()
            else {
                break;
            };
            current_slot = child.chain_next_slot;
            if !child.active {
                continue;
            }
            if let Some(installed) = self
                .castle_children
                .iter_mut()
                .find(|installed| installed.source_entity_index == child.source_entity_index)
            {
                installed.active = false;
            }
            self.sync_castle_child_map(Level2CastleChild {
                active: false,
                ..child
            });
            self.release_slot(child.source_entity_index, None);
            retired_slots.insert(child.source_entity_index);
        }
        if !retired_slots.is_empty() {
            // `sub_613D0` follows each controller through `sub_57F20`, which
            // removes the source row as it returns the slot to NewEvent. Do
            // the same immediately here. Leaving the inactive typed row in
            // the vector lets the later pass reclaim it after a same-pass
            // allocation has reused the slot, unlinking a live successor.
            self.castle_children
                .retain(|child| !retired_slots.contains(&child.source_entity_index));
        }
        if let Some(castle) = self.castle_entry_mut(castle_slot) {
            castle.controller_chain_slots = [0; 8];
            castle.controller_chain_count = 0;
            castle.controller_chain_head = 0;
            castle.controller_chain_tail = 0;
        }
    }

    pub(super) fn install_owner_mana_row(
        &mut self,
        source_entity_index: u16,
        source_class: u8,
        source_model: u8,
        player_entity_index: u16,
        mana: i32,
    ) {
        if !level2_owner_mana_row_qualifies(source_class, source_model) {
            return;
        }
        self.owner_mana_rows.retain(|row| {
            row.source_entity_index != source_entity_index
                || !level2_owner_mana_row_qualifies(row.source_class, row.source_model)
        });
        self.owner_mana_rows.push(Level2OwnerManaRow {
            source_entity_index,
            source_class,
            source_model,
            player_entity_index,
            mana,
            active: true,
        });
    }

    pub(super) fn select_castle_defender_sphere(
        &self,
        holder_position: Position,
        holder_id_entity_index: u16,
        exclude_one: u16,
        exclude_two: u16,
    ) -> Option<u16> {
        // `UpdateEntities_57730` rebuilds the source model-39 chain by
        // ascending arena slot. Sort explicitly so Vec insertion/reclaim
        // order cannot change the strict-`<` tie winner.
        let mut candidates = self
            .spheres
            .iter()
            .filter(|sphere| {
                sphere.active
                    && !sphere.pending_delete
                    && sphere.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
                    && sphere.player_entity_index == holder_id_entity_index
                    && sphere.control_flags & LEVEL2_MANA_SPHERE_TARGET_SUPPRESSION_BIT == 0
                    && sphere.source_entity_index != exclude_one
                    && sphere.source_entity_index != exclude_two
            })
            .copied()
            .collect::<Vec<_>>();
        candidates.sort_unstable_by_key(|sphere| sphere.source_entity_index);
        let mut best = None;
        let mut best_distance = u32::MAX;
        for sphere in candidates {
            let distance = level2_wrapping_source_distance_3d(holder_position, sphere.position);
            if distance < best_distance {
                best_distance = distance;
                best = Some(sphere.source_entity_index);
            }
        }
        best
    }

    /// Bounded projection of `sub_5FD00`: release only the mana that exceeds
    /// the owner's stage capacity (or all mana for a zero-stage Castle),
    /// preserving source order and the source's per-iteration share update.
    /// The source's subsequent `sub_5FF50` controller walk is represented by
    /// the existing ordered sphere/child vectors; no synthetic child is
    /// allocated here.
    fn release_castle_excess_mana(
        &mut self,
        castle_slot: u16,
        terrain: &SessionTerrain,
        reset_free_slots: &[u16],
    ) {
        let Some(castle) = self.castle_entry(castle_slot).copied() else {
            return;
        };
        let release = if castle.level == 0 {
            castle.mana
        } else if self
            .owner_mana_aggregate(castle.source_id_entity_index)
            .saturating_add(castle.mana)
            > castle.mana_capacity
        {
            // `sub_5FD00` tests the Castle owner's dword-0x13C aggregate plus
            // the Castle payload, but its local release amount is the
            // Castle's payload minus maxMana (not the aggregate total).
            castle.mana.saturating_sub(castle.mana_capacity)
        } else {
            0
        };
        if release <= 0 {
            return;
        }
        let fraction = (release / 1_000).clamp(1, 32);
        if self.free_slots.is_empty() {
            self.rebuild_free_arena(reset_free_slots);
        }
        let sphere_count = fraction.min(i32::try_from(self.free_slots.len()).unwrap_or(i32::MAX));
        if sphere_count == 0 {
            return;
        }
        // `sub_5FD00` starts with the quotient from the committed allocation
        // count. It subtracts that share from its remaining local payload and
        // only then lowers the share for a later iteration; it does not give
        // the final sphere an exact-remainder top-up.
        let mut share = release / sphere_count;
        let mut remaining = release;
        let mut source_rng = self
            .source_rng
            .get(&castle_slot)
            .copied()
            .unwrap_or_else(|| self.new_event_rng(castle_slot));
        let mut released: i32 = 0;
        for _ in 0..sphere_count {
            let Some(sphere_slot) = self.allocate_slot() else {
                break;
            };
            let sphere_seed = self.new_event_rng(sphere_slot);
            let sphere_rng = level2_next_rng16(sphere_seed);
            source_rng = level2_next_rng16(source_rng);
            let radius =
                i16::try_from(source_rng % 0x1400 + 3_840).expect("Castle sphere radius fits i16");
            source_rng = level2_next_rng16(source_rng);
            let yaw = i16::try_from(source_rng & 0x07ff).expect("Castle sphere yaw fits i16");
            let mut sphere_position = castle.position;
            crate::move_entity(&mut sphere_position, yaw, 0, radius);
            let ground = terrain.ground_height(castle.position);
            let height_above_ground = i32::from(castle.position.z) - i32::from(ground);
            let vertical_velocity = level2_mana_sphere_vertical_velocity(height_above_ground);
            let sphere_mana = share;
            remaining = remaining.saturating_sub(sphere_mana);
            released = released.saturating_add(sphere_mana);
            if remaining < share {
                share = remaining;
            }
            let sphere = ManaSphere {
                id: u32::from(sphere_slot),
                source_entity_index: sphere_slot,
                action: 41,
                entity_rng: sphere_rng,
                position: sphere_position,
                velocity_x: 0,
                velocity_y: 0,
                vertical_velocity,
                yaw: 0,
                max_life: 300,
                life: 300,
                mana: sphere_mana,
                particle_index: 53,
                collision_yaw: 28,
                collision_pitch: 28,
                collision_roll: 28,
                collision_fov: 28,
                owner_entity_index: castle.owner_entity_index,
                // `sub_5FD00` writes the Castle root's raw id relation, not
                // the separate presentation owner field.  The defender's
                // `sub_5F810` gate compares against this same id.
                player_entity_index: castle.source_id_entity_index,
                incoming_owner_entity_index: 0,
                incoming_possession_payload: 0,
                ordinary_damage_payload: 0,
                ordinary_damage_owner: 0,
                attraction_source_entity_index: 0,
                attraction_step: 0,
                attraction_distance_high: 0,
                assigned_carrier_id: 0,
                assigned_defender_slot: 0,
                control_flags: 8,
                phase: sphere_slot as u8,
                active: true,
                pending_delete: false,
            };
            self.spheres.push(sphere);
            // `sub_5FD00` finishes the NewEvent row with
            // `CopyEntityPosition_57CF0`, which keeps the rebuilt overflow
            // sphere visible to the source map walk used by the next Castle
            // pass.  The allocator retry may reuse an inactive typed row, so
            // relink it after the source position is committed rather than
            // relying on the stale row's prior topology.
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: sphere_slot,
                    active: true,
                    x: sphere_position.x,
                    y: sphere_position.y,
                })
                .expect("Castle overflow sphere source slot fits the map chain");
            self.refresh_owner_mana_rows();
            self.events.push(Level2EconomyEvent::ManaSphereSpawned {
                sphere_id: sphere.id,
                source_entity_index: castle_slot,
                mana: sphere_mana,
            });
        }
        self.source_rng.insert(castle_slot, source_rng);
        if released != 0 {
            if let Some(castle) = self.castle_entry_mut(castle_slot) {
                castle.mana = castle.mana.wrapping_sub(released);
            }
        }
    }

    /// Port the allocation-independent part of `sub_605E0`: reducing the
    /// current stage capacity by ten percent before `sub_5FD00` releases any
    /// overflow, then restoring the old capacity for the following stage
    /// table. No-free arenas still complete this state transition; only the
    /// optional mana-sphere allocations are allowed to fail.
    fn prepare_castle_stage_destruction(
        &mut self,
        castle_slot: u16,
        terrain: &SessionTerrain,
        reset_free_slots: &[u16],
    ) {
        let Some(snapshot) = self.castle_at(castle_slot).copied() else {
            return;
        };
        if snapshot.level == 0 {
            return;
        }
        let reduced_capacity = snapshot
            .mana_capacity
            .saturating_sub(snapshot.mana_capacity / 10);
        if let Some(castle) = self.castle_entry_mut(castle_slot) {
            castle.mana_capacity = reduced_capacity;
        }
        self.release_castle_excess_mana(castle_slot, terrain, reset_free_slots);
        if let Some(castle) = self.castle_entry_mut(castle_slot) {
            castle.mana_capacity = snapshot.mana_capacity;
        }
        self.events
            .push(Level2EconomyEvent::CastleDestructionSound {
                source_entity_index: castle_slot,
                sound_id: 30,
            });
    }

    /// Rebuild the downgraded model-79 chain from `sub_613D0` immediately
    /// after `sub_605E0` removes a stage. This is independent of model-42
    /// construction and therefore remains observable even when the source
    /// arena could not allocate every optional child.
    fn spawn_castle_controller_chain(
        &mut self,
        snapshot: Level2CastleState,
        level: u8,
        terrain: &SessionTerrain,
    ) {
        let Some((selected_level, source_byte_43)) =
            level2_castle_controller_selection(level, self.castle_controller_coefficients_for_owner(snapshot.owner_entity_index))
        else {
            return;
        };
        let (controller_width, controller_height) =
            level2_castle_controller_dimensions(selected_level);
        let center_x = snapshot.position.x.wrapping_add(128).to_be_bytes()[0];
        let center_y = snapshot.position.y.wrapping_add(128).to_be_bytes()[0];
        let start_x = center_x.wrapping_sub(controller_width / 2);
        let start_y = center_y.wrapping_sub(controller_height / 2);
        let mut controller_slots = [0_u16; 8];
        let mut controller_count = 0_u8;
        let mut previous_child_index: Option<usize> = None;
        let mut previous_slot = snapshot.source_entity_index;
        for &(offset_x, offset_y) in level2_castle_controller_points(selected_level) {
            let Some(stage_slot) = self.allocate_slot() else {
                break;
            };
            let stage_position = Position {
                x: u16::from(start_x.wrapping_add_signed(offset_x)) << 8,
                y: u16::from(start_y.wrapping_add_signed(offset_y)) << 8,
                z: 0,
            };
            let ground = terrain.ground_height(stage_position);
            let stage_child = Level2CastleChild {
                source_entity_index: stage_slot,
                entity_rng: self.new_event_rng(stage_slot),
                id_entity_index: snapshot.source_id_entity_index,
                source_class: 10,
                // `sub_613D0` links controller rows through word32/word34;
                // NewEvent leaves parentId zero for this constructor.
                parent_entity_index: 0,
                owner_entity_index: snapshot.owner_entity_index,
                player_entity_index: 0,
                position: Position {
                    z: ground.saturating_add(if selected_level <= 1 { 384 } else { 224 }),
                    ..stage_position
                },
                axis_position: Position::default(),
                model: 79,
                action: 0x56,
                mana: 0,
                max_mana: 0,
                life: 100_000,
                max_life: 100_000,
                speed: LEVEL2_CASTLE_CHILD_DEFAULT_SPEED,
                target_sphere_slot: 0,
                pending_damage: 0,
                pending_attacker_entity_index: 0,
                damage_attacker_entity_index: 0,
                killer_entity_index: 0,
                source_byte_3e: stage_slot as u8,
                source_byte_39: 0,
                source_byte_3b: 0,
                cave_collision_cooldown: 0,
                source_dword_10: 0,
                source_word_2e: 0,
                construction_target_height: 0,
                source_subspell_index: 0,
                yaw: 0,
                roll: 0,
                visual_index: 0,
                control_flags: 0,
                chain_previous_slot: previous_slot,
                chain_next_slot: 0,
                source_byte_43,
                source_word_4a: u16::from(selected_level),
                active: true,
            };
            self.sync_castle_child_map(stage_child);
            self.castle_children.push(stage_child);
            let child_index = self.castle_children.len() - 1;
            if let Some(previous_index) = previous_child_index {
                self.castle_children[previous_index].chain_next_slot = stage_slot;
            }
            previous_child_index = Some(child_index);
            previous_slot = stage_slot;
            controller_slots[usize::from(controller_count)] = stage_slot;
            controller_count = controller_count.saturating_add(1);
        }
        if let Some(castle) = self.castle_entry_mut(snapshot.source_entity_index) {
            castle.controller_chain_slots = controller_slots;
            castle.controller_chain_count = controller_count;
            castle.controller_chain_head = controller_slots[0];
            castle.controller_chain_tail = if controller_count == 0 {
                0
            } else {
                controller_slots[usize::from(controller_count - 1)]
            };
        }
    }

    /// Apply the `sub_5FF50` holder-side `TransformEntityToManaSphere(false)`
    /// branch. The source advances the dying holder RNG once only when it has
    /// mana, creates one ordinary death-transform sphere, and clears the raw
    /// player relation only on that positive-mana path. A zero-mana holder
    /// retains the raw relation while hidden, and leaves the disabled source
    /// row for the next `UpdateEntities_57730` reclaim boundary.
    fn retire_castle_defender_to_mana(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(child) = self
            .castle_children
            .iter()
            .find(|child| {
                child.active
                    && child.source_entity_index == source_slot
                    && child.source_class == 3
                    && child.model == 3
                    && child.action == 9
                    && child.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
            })
            .copied()
        else {
            return false;
        };
        let source_rng_after_death = if child.mana > 0 {
            level2_next_rng16(child.entity_rng)
        } else {
            child.entity_rng
        };
        if child.mana > 0 {
            let _ = self.spawn_mana_spheres_from_death(
                source_slot,
                child.position,
                child.yaw.cast_signed(),
                source_rng_after_death,
                child.mana,
                child.player_entity_index,
                false,
                terrain,
            );
        }
        for sphere in &mut self.spheres {
            if sphere.assigned_defender_slot == source_slot {
                sphere.assigned_defender_slot = 0;
            }
        }
        if let Some(installed) = self
            .castle_children
            .iter_mut()
            .find(|candidate| candidate.source_entity_index == source_slot)
        {
            installed.entity_rng = source_rng_after_death;
            if child.mana > 0 {
                installed.player_entity_index = 0;
            }
            installed.control_flags |= ENTITY_DRAW_DISABLED_BIT;
        }
        self.refresh_owner_mana_rows();
        true
    }

    /// Bounded `sub_5FF50` maintenance after a Castle stage transition.
    /// Existing model-3 action-9 holders are retained in source order, dead
    /// or excess holders are transformed and hidden for the following-pass
    /// reclaim, and missing slots are replaced until the `sub_60400` stage
    /// count is met. This keeps allocator fallback deterministic without
    /// importing the full combat targeting arbitration; model-15 auxiliary
    /// relation creation and fixed tail bounds are represented below.
    pub(super) fn maintain_castle_defenders(&mut self, castle_slot: u16, terrain: &SessionTerrain) {
        let Some(snapshot) = self.castle_at(castle_slot).copied() else {
            return;
        };
        let target = usize::from(level2_castle_defender_count(snapshot.level));
        // `sub_5FF50` trusts the player's array_0x3C_60 relation slots. It
        // does not discover holders by parentId, and NewEvent leaves that
        // field zero for class-3/model-3 rows. Keep the relation array as the
        // authority so a stale or wrong slot cannot release an unrelated
        // live child merely because it shares a synthetic parent.
        let mut defender_slots = snapshot.defender_slots;
        let mut defenders = Vec::with_capacity(target);
        for index in 0..target {
            let related_slot = defender_slots[index];
            let related_child = (related_slot != 0)
                .then(|| {
                    self.castle_children
                        .iter()
                        .find(|child| {
                            child.active
                                && child.source_entity_index == related_slot
                                && child.source_class == 3
                                && child.model == 3
                                && child.action == 9
                                && child.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
                        })
                        .copied()
                })
                .flatten();
            if let Some(child) = related_child {
                if child.life < 0 {
                    // `sub_5FF50` transforms a valid dead holder and clears
                    // this relation, but does not allocate a replacement in
                    // the same indexed visit.
                    self.retire_castle_defender_to_mana(related_slot, terrain);
                    defender_slots[index] = 0;
                    continue;
                }
                defenders.push(related_slot);
                continue;
            }
            if let Some(slot) = self.allocate_slot() {
                let child = Level2CastleChild {
                    source_entity_index: slot,
                    entity_rng: self.new_event_rng(slot),
                    id_entity_index: snapshot.source_id_entity_index,
                    source_class: 3,
                    parent_entity_index: 0,
                    owner_entity_index: snapshot.owner_entity_index,
                    player_entity_index: snapshot.source_id_entity_index,
                    position: snapshot.position,
                    axis_position: Position::default(),
                    model: 3,
                    action: 9,
                    mana: 0,
                    max_mana: LEVEL2_CASTLE_DEFENDER_MAX_MANA,
                    life: LEVEL2_CASTLE_DEFENDER_MAX_LIFE,
                    max_life: LEVEL2_CASTLE_DEFENDER_MAX_LIFE,
                    speed: LEVEL2_CASTLE_DEFENDER_SPEED,
                    target_sphere_slot: 0,
                    pending_damage: 0,
                    pending_attacker_entity_index: 0,
                    damage_attacker_entity_index: 0,
                    killer_entity_index: 0,
                    source_byte_3e: snapshot.phase,
                    source_byte_39: 0,
                    source_byte_3b: 0,
                    cave_collision_cooldown: 0,
                    source_dword_10: 0,
                    source_word_2e: 0,
                    construction_target_height: 0,
                    source_subspell_index: 0,
                    yaw: 0,
                    roll: 0,
                    // sub_4ABA0's class-3/model-3 constructor selects the
                    // balloon particle before sub_5FF50 applies owner color.
                    visual_index: crate::level1_mission::MANA_CARRIER_RENDER_INDEX,
                    control_flags: 0,
                    chain_previous_slot: 0,
                    chain_next_slot: 0,
                    source_byte_43: 0,
                    source_word_4a: 0,
                    active: true,
                };
                self.sync_castle_child_map(child);
                self.castle_children.push(child);
                defender_slots[index] = slot;
                defenders.push(slot);
            } else {
                defender_slots[index] = 0;
            }
        }
        // The source walks the unused array_0x3C entries and transforms valid
        // excess holders before clearing each relation. A wrong relation is
        // deliberately not freed here.
        for index in target..defender_slots.len() {
            let slot = defender_slots[index];
            if slot == 0 {
                continue;
            }
            let valid = self.castle_children.iter().any(|child| {
                child.active
                    && child.source_entity_index == slot
                    && child.source_class == 3
                    && child.model == 3
                    && child.action == 9
                    && child.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
            });
            if valid {
                self.retire_castle_defender_to_mana(slot, terrain);
            }
            defender_slots[index] = 0;
        }
        if let Some(castle) = self.castle_entry_mut(castle_slot) {
            castle.defender_slots = defender_slots;
            castle.defender_count = defenders.len() as u8;
        }

        // `sub_5FF50` resets the owner's aggregate defender mana/max counters
        // and then arbitrates each holder's `word_0x96_150` target. A full
        // Castle first claims the holder; otherwise an eligible action-9
        // holder receives the typed `sub_5F810` model-39 chain selection.
        let mut defender_targets = snapshot.defender_target_spheres;
        let mut defender_total_mana = 0_i32;
        let mut defender_total_max_mana = 0_i32;
        // `sub_60400` supplies fixed v21 for this source visit. Do not
        // compress the loop after a dead indexed holder is cleared: v21 also
        // gates every holder with the parent Castle phase, while the fixed
        // source index still owns the modulo-3 exclusion relations.
        let defender_target_count = target.min(defender_slots.len());
        for index in 0..defender_target_count {
            let slot = defender_slots[index];
            if slot == 0 {
                continue;
            }
            let Some(child) = self
                .castle_children
                .iter()
                .find(|child| child.source_entity_index == slot && child.active)
                .copied()
            else {
                continue;
            };
            // Source leaves word96 untouched when cadence/action/mana gates
            // fail; begin with the relation already installed on the child.
            let mut target_slot = child.target_sphere_slot;
            // `sub_5FF50` gates on the owner's class-10/model-45 aggregate
            // (`dword_0x13C_316`), not the defender total rebuilt below in
            // `dword_0x12E_302`. A full defender still falls through to the
            // Castle relation before the sphere selector is skipped.
            if self
                .owner_mana_aggregate(snapshot.source_id_entity_index)
                .saturating_add(snapshot.mana)
                >= snapshot.mana_capacity
            {
                target_slot = castle_slot;
            } else if snapshot.phase % u8::try_from(defender_target_count.max(1)).unwrap_or(1) == 0
                && child.action == 9
            {
                // Source installs the Castle relation before calling
                // `sub_5F810` or testing a full holder. A missing candidate
                // or a holder at max mana therefore remains pointed at the
                // Castle, not at a stale prior row.
                target_slot = castle_slot;
                if child.mana < child.max_mana {
                    let exclude_one = defender_targets[(index + 1) % 3];
                    let exclude_two = defender_targets[(index + 2) % 3];
                    if let Some(sphere_slot) = self.select_castle_defender_sphere(
                        child.position,
                        child.id_entity_index,
                        exclude_one,
                        exclude_two,
                    ) {
                        target_slot = sphere_slot;
                    }
                }
            }
            if let Some(installed) = self
                .castle_children
                .iter_mut()
                .find(|installed| installed.source_entity_index == slot)
            {
                installed.target_sphere_slot = target_slot;
            }
            defender_targets[index] = target_slot;
            defender_total_mana = defender_total_mana.saturating_add(child.mana);
            defender_total_max_mana = defender_total_max_mana.saturating_add(child.max_mana);
        }
        if let Some(castle) = self.castle_entry_mut(castle_slot) {
            castle.defender_total_mana = defender_total_mana;
            castle.defender_total_max_mana = defender_total_max_mana;
            castle.defender_target_spheres = defender_targets;
        }

        // The source then walks `array_0x5C_92`, first decrementing the root
        // word-0x2C cooldown once per call.  Existing relations are valid
        // only for class 5/model 15 rows whose action is not the delete
        // sentinel 125; an invalid relation clears only the root slot and
        // restarts the 16-frame creation cooldown.
        if let Some(castle) = self.castle_entry_mut(castle_slot) {
            castle.auxiliary_cooldown = castle.auxiliary_cooldown.saturating_sub(1);
        }
        // `sub_60400` supplies the loop bound for `array_0x5C_92`; it is the
        // authored stage tail count, not the number of currently valid rows.
        // This matters when downgrading from a larger stage: source leaves
        // tail entries beyond the new bound untouched.
        let auxiliary_target = usize::from(level2_castle_auxiliary_count(snapshot.level));
        let mut auxiliary_slots = snapshot.auxiliary_slots;
        let mut auxiliary_positions = snapshot.auxiliary_positions;
        let mut auxiliary_yaws = snapshot.auxiliary_yaws;
        let mut auxiliary_rolls = snapshot.auxiliary_rolls;
        let mut auxiliary_players = snapshot.auxiliary_player_entity_indices;
        let mut cooldown = self
            .castle_entry(castle_slot)
            .map(|castle| castle.auxiliary_cooldown)
            .unwrap_or_default();
        for index in 0..auxiliary_target {
            let slot = auxiliary_slots[index];
            if slot == 0 {
                if cooldown == 0 {
                    if let Some(slot) = self.allocate_slot() {
                        let rng = self.new_event_rng(slot);
                        let mut child = creatures::creature_15::CastleDefenderState::construct(
                            slot,
                            self.slot_generation(slot),
                            rng,
                            snapshot.position,
                            &mut self.castle_auxiliary_constructor_phase,
                            creatures::creature_15::DefenderOrigin::Castle(castle_slot),
                        );
                        child.castle_owner =
                            Some(crate::campaign::source_arena::SourceEntityHandle {
                                source_slot: castle_slot,
                                generation: self.slot_generation(castle_slot),
                            });
                        child.bind_castle(snapshot.source_id_entity_index, terrain);
                        self.source_owner
                            .insert(slot, snapshot.source_id_entity_index);
                        self.source_rng.insert(slot, rng);
                        self.map_chains
                            .sync_entity(RenderableMapSeed {
                                entity_index: slot,
                                active: true,
                                x: child.position.x,
                                y: child.position.y,
                            })
                            .expect("Castle defender source slot fits map chain");
                        self.castle_defenders.push(child);
                        auxiliary_slots[index] = slot;
                        auxiliary_positions[index] = child.position;
                        auxiliary_yaws[index] = child.yaw;
                        auxiliary_rolls[index] = child.roll;
                        auxiliary_players[index] = child.player_entity_index;
                        cooldown = 16;
                    }
                }
                continue;
            }
            let valid = self.castle_defenders.iter().any(|child| {
                child.active && child.source_entity_index == slot && child.action != 125
            });
            if valid {
                continue;
            }

            // `sub_5FF50` clears the relation and cooldown only. It does not
            // release an unrelated live row merely because the relation is
            // stale or has the wrong class/model/action; ApplyEvents and the
            // next allocator episode own that row's eventual reclamation.
            auxiliary_slots[index] = 0;
            auxiliary_positions[index] = Position::default();
            auxiliary_yaws[index] = 0;
            auxiliary_rolls[index] = 0;
            auxiliary_players[index] = 0;

            cooldown = 16;
        }
        let auxiliary_count = auxiliary_target as u8;
        if let Some(castle) = self.castle_entry_mut(castle_slot) {
            castle.auxiliary_slots = auxiliary_slots;
            castle.auxiliary_count = auxiliary_count;
            castle.auxiliary_cooldown = cooldown;
            castle.auxiliary_positions = auxiliary_positions;
            castle.auxiliary_yaws = auxiliary_yaws;
            castle.auxiliary_rolls = auxiliary_rolls;
            castle.auxiliary_player_entity_indices = auxiliary_players;
        }
    }

    /// Advance the bounded Castle root action. This follows the source state
    /// table in `BeginOfCastleCreation_5FA70`: states 0/3 allocate a class-10
    /// model-42 child, state 4 scans for that child, and state 5 allocates the
    /// model-41 child. The model-42 allocation is atomic; the bounded
    /// model-79 stage-controller loop may commit a partial chain when the
    /// source arena exhausts.
    pub fn step_castle_action(
        &mut self,
        castle_slot: u16,
        terrain: &mut SessionTerrain,
        reset_free_slots: &[u16],
    ) -> bool {
        self.step_castle_action_with_final_loss(castle_slot, terrain, reset_free_slots, &mut |_, _| {})
    }

    /// The caller owns player inventory and raw holder rows. Invoke its
    /// final-loss operation at the source point inside `sub_605E0`, before
    /// the Castle relation/root are removed and the `sub_5FCA0` tail runs.
    pub(crate) fn step_castle_action_with_final_loss(
        &mut self,
        castle_slot: u16,
        terrain: &mut SessionTerrain,
        reset_free_slots: &[u16],
        on_final_loss: &mut dyn FnMut(&mut Level2EconomyState, Level2CastleState),
    ) -> bool {
        let Some(snapshot) = self.castle_at(castle_slot).copied() else {
            return false;
        };
        // EndOfCastleProjectile_5F8F0 writes this relation on every
        // zero-timer action-4 visit, including lethal and upgrade exits.
        if snapshot.action == 4 && snapshot.timer == 0 {
            self.castle_entry_mut(castle_slot)
                .expect("Castle remains installed")
                .player_entity_index = snapshot.source_id_entity_index;
        }
        match snapshot.action {
            5 if snapshot.build_state == 0 || snapshot.build_state == 3 => {
                let source_construction_start = snapshot.level == 0
                    && snapshot.source_word_2e == 0
                    && snapshot.source_class == 3
                    && snapshot.source_model == 2;
                let Some(child_slot) = self.allocate_slot() else {
                    if !source_construction_start {
                        self.advance_castle_phase(castle_slot);
                    }
                    return false;
                };
                let next_level = snapshot.level.saturating_add(1).max(1);
                let (base_life, stage_mana_capacity) = level2_castle_stage_stats(next_level);
                let stage_coefficient = self.castle_stage_coefficient_for_owner(snapshot.owner_entity_index, next_level);
                let stage_life =
                    i32::try_from((i64::from(base_life) * i64::from(stage_coefficient)) >> 8)
                        .unwrap_or(if base_life >= 0 { i32::MAX } else { i32::MIN });
                let (shift_pitch, shift_roll) = level2_castle_shift_axes(next_level);
                self.retire_castle_controller_chain(snapshot.source_entity_index);
                let construction_child = Level2CastleChild {
                    source_entity_index: child_slot,
                    entity_rng: self.new_event_rng(child_slot),
                    id_entity_index: snapshot.source_id_entity_index,
                    source_class: 10,
                    parent_entity_index: snapshot.source_entity_index,
                    owner_entity_index: snapshot.owner_entity_index,
                    player_entity_index: 0,
                    position: snapshot.anchor_position,
                    axis_position: Position::default(),
                    model: 42,
                    action: 44,
                    mana: 0,
                    max_mana: 0,
                    life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                    max_life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                    speed: LEVEL2_CASTLE_CHILD_DEFAULT_SPEED,
                    target_sphere_slot: 0,
                    pending_damage: 0,
                    pending_attacker_entity_index: 0,
                    damage_attacker_entity_index: 0,
                    killer_entity_index: 0,
                    source_byte_3e: child_slot as u8,
                    source_byte_39: 0,
                    // `sub_50370` sets byte 0x3B before dispatching 37BC0;
                    // that selects the -25 negative hold after BUILD pass 1.
                    source_byte_3b: 1,
                    cave_collision_cooldown: next_level,
                    source_dword_10: 0,
                    source_word_2e: 0,
                    construction_target_height: 0,
                    source_subspell_index: 0,
                    yaw: 0,
                    roll: 0,
                    visual_index: 0,
                    // Source flags retain bit 0 and 37BC0 adds bit 2 on its
                    // first callback.
                    control_flags: 1,
                    chain_previous_slot: 0,
                    chain_next_slot: 0,
                    source_byte_43: 0,
                    source_word_4a: 0,
                    active: true,
                };
                self.castle_children.push(construction_child);
                // `sub_613D0` removes the prior `word_0x34_52` chain, then
                // allocates the source-ordered model-79 controller points
                // from x_BYTE_DB038.  NewEvent failure is deliberately
                // non-atomic: model-42 and any committed controller prefix
                // remain installed, exactly as in the source loop.
                let mut controller_slots = [0_u16; 8];
                let mut controller_count = 0_u8;
                if let Some((selected_level, source_byte_43)) = level2_castle_controller_selection(
                    next_level,
                    self.castle_controller_coefficients_for_owner(snapshot.owner_entity_index),
                ) {
                    let (controller_width, controller_height) =
                        level2_castle_controller_dimensions(selected_level);
                    let center_x = snapshot.position.x.wrapping_add(128).to_be_bytes()[0];
                    let center_y = snapshot.position.y.wrapping_add(128).to_be_bytes()[0];
                    let start_x = center_x.wrapping_sub(controller_width / 2);
                    let start_y = center_y.wrapping_sub(controller_height / 2);
                    let mut previous_child_index: Option<usize> = None;
                    let mut previous_slot = snapshot.source_entity_index;
                    for &(offset_x, offset_y) in level2_castle_controller_points(selected_level) {
                        let Some(stage_slot) = self.allocate_slot() else {
                            break;
                        };
                        let stage_position = Position {
                            x: u16::from(start_x.wrapping_add_signed(offset_x)) << 8,
                            y: u16::from(start_y.wrapping_add_signed(offset_y)) << 8,
                            z: 0,
                        };
                        let ground = terrain.ground_height(stage_position);
                        let stage_child = Level2CastleChild {
                            source_entity_index: stage_slot,
                            entity_rng: self.new_event_rng(stage_slot),
                            id_entity_index: snapshot.source_id_entity_index,
                            source_class: 10,
                            // `sub_613D0` exposes the controller ownership
                            // only through word32/word34; NewEvent leaves
                            // parentId zero.
                            parent_entity_index: 0,
                            owner_entity_index: snapshot.owner_entity_index,
                            player_entity_index: 0,
                            position: Position {
                                z: ground.saturating_add(if selected_level <= 1 {
                                    384
                                } else {
                                    224
                                }),
                                ..stage_position
                            },
                            axis_position: Position::default(),
                            model: 79,
                            action: 0x56,
                            mana: 0,
                            max_mana: 0,
                            life: 100_000,
                            max_life: 100_000,
                            speed: LEVEL2_CASTLE_CHILD_DEFAULT_SPEED,
                            target_sphere_slot: 0,
                            pending_damage: 0,
                            pending_attacker_entity_index: 0,
                            damage_attacker_entity_index: 0,
                            killer_entity_index: 0,
                            source_byte_3e: stage_slot as u8,
                            source_byte_39: 0,
                            source_byte_3b: 0,
                            cave_collision_cooldown: 0,
                            source_dword_10: 0,
                            source_word_2e: 0,
                            construction_target_height: 0,
                            source_subspell_index: 0,
                            yaw: 0,
                            roll: 0,
                            visual_index: 0,
                            control_flags: 0,
                            chain_previous_slot: previous_slot,
                            chain_next_slot: 0,
                            source_byte_43,
                            source_word_4a: u16::from(selected_level),
                            active: true,
                        };
                        self.sync_castle_child_map(stage_child);
                        self.castle_children.push(stage_child);
                        let child_index = self.castle_children.len() - 1;
                        if let Some(previous_index) = previous_child_index {
                            self.castle_children[previous_index].chain_next_slot = stage_slot;
                        }
                        previous_child_index = Some(child_index);
                        previous_slot = stage_slot;
                        controller_slots[usize::from(controller_count)] = stage_slot;
                        controller_count = controller_count.saturating_add(1);
                    }
                }
                let castle = self.castle_entry_mut(castle_slot).expect("Castle remains installed");
                castle.construction_slot = child_slot;
                castle.construction_model = 42;
                castle.construction_action = 44;
                castle.construction_control_flags = 1;
                castle.construction_parent_slot = castle.source_entity_index;
                castle.construction_owner_entity_index = castle.owner_entity_index;
                castle.construction_id = castle.source_id_entity_index;
                castle.construction_position = castle.anchor_position;
                castle.child_chain_slots[0] = child_slot;
                castle.child_chain_models[0] = 42;
                castle.child_chain_active[0] = true;
                castle.controller_chain_slots = controller_slots;
                castle.controller_chain_count = controller_count;
                castle.controller_chain_head = controller_slots[0];
                castle.controller_chain_tail = if controller_count == 0 {
                    0
                } else {
                    controller_slots[usize::from(controller_count - 1)]
                };
                castle.construction_child_active = true;
                castle.build_state = 4;
                castle.level = next_level;
                castle.life = stage_life;
                castle.max_life = stage_life;
                castle.mana_capacity = stage_mana_capacity;
                castle.shift_pitch = shift_pitch;
                castle.shift_roll = shift_roll;
                // `sub_60480` writes the completed Castle shift envelope at
                // the successful child allocation, before the action-5
                // construction callback begins.
                castle.shift_yaw = -8_192;
                castle.shift_fov = 0x4000;
                castle.control_flags &= !0x40;
                if source_construction_start {
                    // `sub_60480` commits the construction relation only
                    // after its child allocation succeeds.
                    castle.source_word_2e = 4;
                }
                let completed_slot = castle.source_entity_index;
                let completed_level = castle.level;
                let owner = castle.owner_entity_index;
                self.link_castle_owner(owner, completed_slot);
                if owner == self.local_player_entity_index {
                    self.player_castle_entity_index = completed_slot;
                    self.player_castle_stage = completed_level;
                    self.player_castle_aux_flags = 0;
                }
                self.events
                    .push(Level2EconomyEvent::CastleConstructionSound {
                        source_entity_index: completed_slot,
                        sound_id: 10,
                    });
                self.advance_castle_phase(castle_slot);
                true
            }
            5 if snapshot.build_state == 2 => {
                let castle = self.castle_entry_mut(castle_slot).expect("Castle remains installed");
                castle.action = 4;
                castle.build_state = 0;
                castle.shift_yaw = -8_192;
                castle.shift_fov = 0x4000;
                self.advance_castle_phase(castle_slot);
                true
            }
            5 if snapshot.build_state == 1 => {
                let castle = self.castle_entry_mut(castle_slot).expect("Castle remains installed");
                castle.position.z = terrain.ground_height(castle.position);
                self.advance_castle_phase(castle_slot);
                true
            }
            5 if snapshot.build_state == 4 => {
                if self
                    .castle_children
                    .iter()
                    .any(|child| child.active && child.model == 42)
                {
                    self.advance_castle_phase(castle_slot);
                    return true;
                }
                self.castle_entry_mut(castle_slot)
                    .expect("Castle remains installed")
                    .build_state = 3;
                self.advance_castle_phase(castle_slot);
                true
            }
            5 if snapshot.build_state == 5 => {
                let Some(child_slot) = self.allocate_slot() else {
                    self.advance_castle_phase(castle_slot);
                    return false;
                };
                let defense_child = Level2CastleChild {
                    source_entity_index: child_slot,
                    entity_rng: self.new_event_rng(child_slot),
                    id_entity_index: snapshot.source_id_entity_index,
                    source_class: 10,
                    parent_entity_index: snapshot.source_entity_index,
                    owner_entity_index: snapshot.owner_entity_index,
                    player_entity_index: 0,
                    position: snapshot.anchor_position,
                    axis_position: Position::default(),
                    model: 41,
                    action: 43,
                    mana: 0,
                    max_mana: 0,
                    life: 100_000,
                    max_life: 100_000,
                    speed: LEVEL2_CASTLE_CHILD_DEFAULT_SPEED,
                    target_sphere_slot: 0,
                    pending_damage: 0,
                    pending_attacker_entity_index: 0,
                    damage_attacker_entity_index: 0,
                    killer_entity_index: 0,
                    source_byte_3e: child_slot as u8,
                    source_byte_39: 0,
                    source_byte_3b: 0,
                    cave_collision_cooldown: snapshot.level.max(1),
                    source_dword_10: 0,
                    source_word_2e: 0,
                    construction_target_height: 0,
                    source_subspell_index: 0,
                    yaw: 0,
                    roll: 0,
                    visual_index: 0,
                    control_flags: 0,
                    chain_previous_slot: 0,
                    chain_next_slot: 0,
                    source_byte_43: 0,
                    source_word_4a: 0,
                    active: true,
                };
                self.sync_castle_child_map(defense_child);
                self.castle_children.push(defense_child);
                let castle = self.castle_entry_mut(castle_slot).expect("Castle remains installed");
                castle.defense_slot = child_slot;
                castle.defense_model = 41;
                castle.defense_action = 43;
                castle.defense_control_flags = 0;
                castle.defense_parent_slot = castle.source_entity_index;
                castle.defense_owner_entity_index = castle.owner_entity_index;
                castle.defense_id = castle.source_id_entity_index;
                castle.defense_position = castle.anchor_position;
                castle.child_chain_slots[1] = child_slot;
                castle.child_chain_models[1] = 41;
                castle.child_chain_active[1] = true;
                castle.defense_child_active = true;
                castle.build_state = 6;
                self.advance_castle_phase(castle_slot);
                true
            }
            5 if snapshot.build_state == 6 => {
                self.castle_entry_mut(castle_slot)
                    .expect("Castle remains installed")
                    .position
                    .z = terrain.ground_height(snapshot.position);
                self.advance_castle_phase(castle_slot);
                true
            }
            4 if snapshot.source_word_80 == snapshot.source_id_entity_index
                && snapshot.source_word_7c == 10 =>
            {
                let castle = self.castle_entry_mut(castle_slot).expect("Castle remains installed");
                castle.source_word_80 = 0;
                castle.source_word_7c = 0;
                castle.control_flags |= 0x40;
                castle.source_word_2e = 0;
                castle.action = 5;
                castle.build_state = 0;
                // The 10/43 hit is consumed by this action-4 follow-through;
                // only after the mailbox match does source 5F890 release the
                // shared family-2 holder.
                castle.upgrade_holder_clear_pending = true;
                self.advance_castle_phase(castle_slot);
                true
            }
            4 if snapshot.timer > 0 => {
                let castle = self.castle_entry_mut(castle_slot).expect("Castle remains installed");
                if castle.timer == 1 {
                    castle.timer = 0;
                    if castle.control_flags & 0x10 == 0 {
                        castle.action = 5;
                        castle.build_state = 3;
                    }
                } else {
                    castle.timer = castle.timer.wrapping_sub(1);
                }
                castle.position.z = terrain.ground_height(castle.position);
                self.advance_castle_phase(castle_slot);
                true
            }
            4 if snapshot.life < 0 => {
                self.castle_entry_mut(castle_slot)
                    .expect("Castle remains installed")
                    .action = 6;
                self.advance_castle_phase(castle_slot);
                true
            }
            4 if snapshot.control_flags & 0x40 != 0 => {
                let castle = self.castle_entry_mut(castle_slot).expect("Castle remains installed");
                castle.control_flags &= !0x40;
                castle.build_state = 0;
                castle.action = 5;
                self.advance_castle_phase(castle_slot);
                true
            }
            4 => {
                let source_phase = snapshot.phase;
                if snapshot.pending_attacker_entity_index != 0 {
                    let _ = self.resolve_damage(castle_slot);
                    if self.castle_entry(castle_slot).is_some_and(|castle| castle.life < 0) {
                        self.castle_entry_mut(castle_slot)
                            .expect("Castle remains installed")
                            .action = 6;
                        self.advance_castle_phase(castle_slot);
                        return true;
                    }
                    if self
                        .castle_entry(castle_slot)
                        .is_some_and(|castle| castle.control_flags & 0x40 != 0)
                    {
                        let castle = self
                            .castle_entry_mut(castle_slot)
                            .expect("Castle remains installed");
                        castle.action = 5;
                        castle.build_state = 0;
                        castle.position.z = terrain.ground_height(castle.position);
                        self.advance_castle_phase(castle_slot);
                        return true;
                    }
                }
                if source_phase & 1 != 0 {
                    self.advance_castle_phase(castle_slot);
                    return true;
                }
                if let Some(castle) = self.castle_entry_mut(castle_slot) {
                    castle.position.z = terrain.ground_height(castle.position);
                }
                self.release_castle_excess_mana(castle_slot, terrain, reset_free_slots);
                self.maintain_castle_defenders(castle_slot, terrain);
                let Some(castle) = self.castle_entry(castle_slot).copied() else {
                    self.advance_castle_phase(castle_slot);
                    return false;
                };
                if castle.mana >= castle.mana_capacity {
                    self.advance_castle_phase(castle_slot);
                    return true;
                }
                let Some(sphere_index) = self.spheres.iter().position(|sphere| {
                    sphere.active
                        && !sphere.pending_delete
                        && sphere.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
                        && sphere.owner_entity_index == castle.owner_entity_index
                        && level2_castle_sphere_overlap(castle.position, sphere)
                }) else {
                    self.advance_castle_phase(castle_slot);
                    return true;
                };
                let sphere = self.spheres[sphere_index];
                let castle = self.castle_entry_mut(castle_slot).expect("Castle remains installed");
                castle.mana = castle.mana.wrapping_add(sphere.mana);
                let carrier_id = u32::from(castle.source_entity_index);
                self.spheres[sphere_index].pending_delete = true;
                self.spheres[sphere_index].control_flags |= ENTITY_DRAW_DISABLED_BIT;
                self.events.push(Level2EconomyEvent::ManaSphereCollected {
                    sphere_id: sphere.id,
                    carrier_id,
                    mana: sphere.mana,
                });
                self.advance_castle_phase(castle_slot);
                true
            }
            6 => {
                // `sub_5FCA0` gates the whole teardown on
                // `sub_4A810_get_0x35plus()`: with no free arena slot the
                // source only changes action back to 4, leaving stage,
                // terrain, and economy state untouched.
                if self.free_slots.is_empty() {
                    if let Some(castle) = self.castle_entry_mut(castle_slot) {
                        castle.action = 4;
                    }
                    // `ApplyEvents_498A0` advances byte 0x3E after
                    // `sub_5FCA0`, including this no-free callback path.
                    self.advance_castle_phase(castle_slot);
                    return true;
                }
                // Keep the source order: reduce capacity and release
                // overflow before the destruction sound and stage terrain
                // removal (`sub_605E0` -> `sub_5FD00` -> `RemoveCastleStage`).
                self.prepare_castle_stage_destruction(castle_slot, terrain, reset_free_slots);
                if snapshot.level > 0 {
                    // The source sentinel carries the *old* Castle level in
                    // byte 0x46; index that DATA_BUILD record rather than
                    // always reusing stage one's footprint.
                    if let Some(footprint) =
                        self.castle_stage_footprints.get(&snapshot.level).cloned()
                    {
                        // `sub_605E0` copies the Castle `axis_0x9A` into the
                        // player entity before `RemoveCastleStage_385C0`.
                        // That is the construction anchor, not the snapped
                        // root position used by map linking.
                        let center_x =
                            snapshot.anchor_position.x.wrapping_add(128).to_be_bytes()[0];
                        let center_y =
                            snapshot.anchor_position.y.wrapping_add(128).to_be_bytes()[0];
                        let start_x = center_x.wrapping_sub(footprint.width / 2);
                        let start_y = center_y.wrapping_sub(footprint.height / 2);
                        // `sub_605E0` repurposes entity[0] as the stage-removal
                        // sentinel. `RemoveCastleStage_385C0` advances that
                        // row's rand_0x14; the Castle root RNG was consumed
                        // only by the preceding `sub_5FD00` overflow path.
                        self.sentinel_rng = remove_castle_stage(
                            &mut terrain.planes,
                            start_x,
                            start_y,
                            &footprint,
                            terrain.map_type,
                            &mut terrain.terrain_rng,
                            self.sentinel_rng,
                        );
                    }
                }
                // `sub_613D0` retires only the old controller chain.  The
                // source-owned defender and model-15 arrays are consumed by
                // the following `sub_5FF50` maintenance, so parent identity
                // is deliberately not a teardown release criterion.
                self.retire_castle_controller_chain(snapshot.source_entity_index);
                let next_level = snapshot.level.saturating_sub(1);
                let (base_life, stage_mana_capacity) = level2_castle_stage_stats(next_level);
                let stage_coefficient = self.castle_stage_coefficient_for_owner(snapshot.owner_entity_index, next_level);
                let stage_life =
                    i32::try_from((i64::from(base_life) * i64::from(stage_coefficient)) >> 8)
                        .unwrap_or(if base_life >= 0 { i32::MAX } else { i32::MIN });
                let (shift_pitch, shift_roll) = level2_castle_shift_axes(next_level);
                let recovered_life = if next_level == 0 {
                    // sub_60780 receives number1=0 for stage zero and
                    // consequently leaves both life fields unchanged.
                    snapshot.life
                } else {
                    let deficit = if snapshot.life < 0 {
                        snapshot.life.saturating_neg().min(stage_life / 2)
                    } else {
                        0
                    };
                    stage_life.saturating_sub(deficit)
                };
                let remaining_mana = self
                    .castle_entry(castle_slot)
                    .map(|castle| castle.mana)
                    .unwrap_or_default();
                let castle = self.castle_entry_mut(castle_slot).expect("Castle remains installed");
                castle.build_state = 0;
                castle.level = next_level;
                castle.life = recovered_life;
                castle.max_life = if next_level == 0 { snapshot.max_life } else { stage_life };
                castle.mana = remaining_mana;
                castle.mana_capacity = stage_mana_capacity;
                castle.construction_slot = 0;
                castle.defense_slot = 0;
                castle.construction_model = 0;
                castle.construction_action = 0;
                castle.construction_control_flags = 0;
                castle.construction_parent_slot = 0;
                castle.construction_owner_entity_index = 0;
                castle.construction_id = 0;
                castle.construction_position = Position::default();
                castle.defense_model = 0;
                castle.defense_action = 0;
                castle.defense_control_flags = 0;
                castle.defense_parent_slot = 0;
                castle.defense_owner_entity_index = 0;
                castle.defense_id = 0;
                castle.defense_position = Position::default();
                castle.child_chain_slots = [0; 2];
                castle.child_chain_models = [0; 2];
                castle.child_chain_active = [false; 2];
                castle.controller_chain_slots = [0; 8];
                castle.controller_chain_count = 0;
                castle.controller_chain_head = 0;
                castle.controller_chain_tail = 0;
                castle.shift_pitch = shift_pitch;
                castle.shift_roll = shift_roll;
                castle.shift_yaw = -8_192;
                castle.shift_fov = 0x4000;
                // `sub_605E0` leaves the source auxiliary array intact;
                // `sub_5FF50` only walks the lower stage bound, so tail rows
                // remain source-owned even when the Castle downgrades.
                castle.auxiliary_slots = snapshot.auxiliary_slots;
                castle.auxiliary_count = snapshot.auxiliary_count;
                castle.auxiliary_cooldown = snapshot.auxiliary_cooldown;
                castle.auxiliary_positions = snapshot.auxiliary_positions;
                castle.auxiliary_yaws = snapshot.auxiliary_yaws;
                castle.auxiliary_rolls = snapshot.auxiliary_rolls;
                castle.auxiliary_player_entity_indices = snapshot.auxiliary_player_entity_indices;
                castle.construction_child_active = false;
                castle.defense_child_active = false;
                let destroyed_slot = castle.source_entity_index;
                let killer = castle.killer_entity_index;
                let castle_level = castle.level;
                let owner = castle.owner_entity_index;
                if castle_level == 0 {
                    let final_castle = *castle;
                    on_final_loss(self, final_castle);
                }
                self.events.push(Level2EconomyEvent::CastleStageDestroyed {
                    source_entity_index: destroyed_slot,
                    previous_level: snapshot.level,
                    next_level,
                });
                if castle_level == 0 {
                    if self.castle_links.get(&owner).is_some_and(|link| link.1 == destroyed_slot) {
                        self.castle_links.remove(&owner);
                    }
                    if owner == self.local_player_entity_index {
                        self.player_castle_entity_index = 0;
                        self.player_castle_stage = 0;
                        self.player_castle_aux_flags = 0;
                        if self.player_model == 1 && self.player_terrain_flags & 4 != 0 {
                            self.player_spell_slot_2 = 0;
                            self.player_spell_holder_action = 0;
                        }
                    }
                    self.castle_entry_mut(castle_slot)
                        .expect("Castle remains installed during final-loss callback")
                        .active = false;
                    self.events.push(Level2EconomyEvent::CastleDestroyed {
                        source_entity_index: destroyed_slot,
                        killer_entity_index: killer,
                    });
                } else if owner == self.local_player_entity_index {
                    self.player_castle_stage = castle_level;
                    self.player_castle_aux_flags = 0;
                }
                // `sub_5F890(a1, 1)` copies the holder's own max counter;
                // it is not derived from the Castle stage number.  The final
                // non-player path calls `sub_5F890(a1, 0)` and clears it,
                // while a model-1 player on terrain flag 4 disables the
                // holder relation entirely.
                if owner == self.local_player_entity_index && self.player_spell_slot_2 != 0 {
                    if next_level > 0 {
                        self.player_spell_holder_action =
                            self.player_spell_holder_max_action.saturating_sub(1);
                    } else {
                        self.player_spell_holder_action = 0;
                    }
                }
                if next_level > 0 {
                    let downgraded = *self.castle_entry(castle_slot).expect("Castle remains installed");
                    self.spawn_castle_controller_chain(downgraded, next_level, terrain);
                }
                // sub_5FCA0 resumes only after sub_605E0 and its final-loss
                // owner operation have completed.
                if let Some(castle) = self.castle_entry_mut(castle_slot) {
                    castle.action = 4;
                }
                // `sub_5FCA0` performs a second overflow release and then
                // runs `sub_5FF50` after `sub_605E0` has rebuilt the lower
                // stage. Allocation failure here must not roll back the
                // already-committed stage transition.
                self.release_castle_excess_mana(castle_slot, terrain, reset_free_slots);
                self.maintain_castle_defenders(castle_slot, terrain);
                if let Some(castle) = self.castle_entry_mut(castle_slot) {
                    // `sub_5FCA0` clears word-0x2E and leaves word-0x30 at
                    // the post-downgrade timer sentinel 5.
                    castle.source_word_2e = 0;
                    castle.source_word_30 = 5;
                    castle.timer = 5;
                }
                if let Some(castle) = self
                    .castle_entry(castle_slot)
                    .copied()
                    .filter(|castle| !castle.active)
                {
                    self.map_chains
                        .sync_entity(RenderableMapSeed {
                            entity_index: castle.source_entity_index,
                            active: false,
                            x: castle.position.x,
                            y: castle.position.y,
                        })
                        .expect("Castle root source slot fits the map chain");
                }
                self.advance_castle_phase(castle_slot);
                true
            }
            _ => {
                // The outer source entity dispatcher advances byte 0x3E
                // after every action call, including unhandled actions.
                self.advance_castle_phase(castle_slot);
                false
            }
        }
    }

    pub fn return_carrier_to_castle(&mut self, carrier_slot: u16) -> bool {
        let Some(owner) = self.carriers.iter().find(|carrier| carrier.active && carrier.source_entity_index == carrier_slot).map(|carrier| carrier.owner_entity_index) else {
            return false;
        };
        let Some(castle_slot) = self.castle_for_owner(owner).map(|castle| castle.source_entity_index) else {
            return false;
        };
        let Some(carrier) = self
            .carriers
            .iter_mut()
            .find(|carrier| carrier.active && carrier.source_entity_index == carrier_slot)
        else {
            return false;
        };
        carrier.target_sphere_id = u32::from(castle_slot);
        carrier.returning_to_castle = true;
        true
    }

    /// Advance a returning carrier to a supplied source position and commit
    /// `AddBallon_60AB0`'s Castle arrival only once the source X/Y distance,
    /// stage, and terrain upper-bound gates pass. The carrier remains active
    /// with its life restored; its mana payload is transferred and cleared at
    /// arrival, not when the return is scheduled.
    pub fn step_carrier_to_castle(
        &mut self,
        carrier_slot: u16,
        position: Position,
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(owner) = self.carriers.iter().find(|carrier| carrier.active && carrier.source_entity_index == carrier_slot).map(|carrier| carrier.owner_entity_index) else {
            return false;
        };
        let Some(castle) = self.castle_for_owner(owner).copied() else {
            return false;
        };
        let raw_position = position;
        let ground = terrain.ground_height(raw_position);
        let Some(carrier) = self
            .carriers
            .iter_mut()
            .find(|carrier| carrier.active && carrier.source_entity_index == carrier_slot)
        else {
            return false;
        };
        if !carrier.returning_to_castle
            || carrier.owner_entity_index != castle.owner_entity_index
            || castle.level == 0
        {
            return false;
        }
        let radius = u32::from(carrier.speed.unsigned_abs()) * u32::from(castle.level);
        let terrain_upper_bound = ground.saturating_add(MANA_CARRIER_COLLISION_VERTICAL_OFFSET);
        if level2_wrapping_xy_distance(raw_position, castle.position) > radius.wrapping_mul(radius)
            || raw_position.z > terrain_upper_bound
        {
            return false;
        }
        let mut position = raw_position;
        if position.z < ground {
            position.z = ground;
        }
        carrier.position = position;
        let mana = carrier.carried_mana;
        carrier.carried_mana = 0;
        carrier.target_sphere_id = 0;
        carrier.returning_to_castle = false;
        carrier.life = carrier.max_life;
        let carrier_id = carrier.id;
        let castle_state = self.castle_at_mut(castle.source_entity_index).expect("Castle remains installed");
        castle_state.mana = castle_state.mana.wrapping_add(mana);
        self.events.push(Level2EconomyEvent::ManaCarrierArrived {
            carrier_id,
            castle_id: castle.source_entity_index,
            mana,
        });
        true
    }

    pub fn submit_damage(&mut self, target_slot: u16, damage: i32, attacker: u16) -> bool {
        if damage <= 0 || attacker == 0 {
            return false;
        }
        if let Some(auxiliary) = self.castle_defenders.iter_mut().find(|child| {
            child.active
                && child.life >= 0
                && child.source_entity_index == target_slot
                && !matches!(child.action, 124 | 125)
        }) {
            crate::combat::actors::DamageMailboxMut::new(
                &mut auxiliary.pending_damage,
                &mut auxiliary.pending_attacker_entity_index,
            )
            .submit(
                damage,
                attacker,
                crate::combat::actors::DamageWriter::Ordinary,
            );
            return true;
        }
        if let Some(carrier) = self.carriers.iter_mut().find(|carrier| {
            carrier.active && carrier.life >= 0 && carrier.source_entity_index == target_slot
        }) {
            crate::combat::actors::DamageMailboxMut::new(
                &mut carrier.pending_damage,
                &mut carrier.pending_attacker_entity_index,
            )
            .submit(
                damage,
                attacker,
                crate::combat::actors::DamageWriter::Ordinary,
            );
            return true;
        }
        if let Some(castle) = self.castle_at_mut(target_slot).filter(|castle| castle.life >= 0) {
            crate::combat::actors::DamageMailboxMut::new(
                &mut castle.pending_damage,
                &mut castle.pending_attacker_entity_index,
            )
            .submit(
                damage,
                attacker,
                crate::combat::actors::DamageWriter::Ordinary,
            );
            return true;
        }
        false
    }

    pub fn resolve_damage(&mut self, target_slot: u16) -> Option<bool> {
        if let Some(carrier) = self
            .carriers
            .iter_mut()
            .find(|carrier| carrier.active && carrier.source_entity_index == target_slot)
        {
            let damage = carrier.pending_damage;
            let attacker = carrier.pending_attacker_entity_index;
            if attacker == 0 {
                return None;
            }
            carrier.life = carrier.life.wrapping_sub(damage);
            carrier.pending_damage = 0;
            carrier.pending_attacker_entity_index = 0;
            let lethal = carrier.life < 0;
            if lethal {
                carrier.active = false;
            }
            self.events.push(Level2EconomyEvent::DamageApplied {
                target_entity_index: target_slot,
                attacker_entity_index: attacker,
                damage,
                lethal,
            });
            return Some(lethal);
        }
        let local_owner = self.local_player_entity_index;
        let castle = self.castle_at_mut(target_slot)?;
        let damage = castle.pending_damage;
        let attacker = castle.pending_attacker_entity_index;
        if attacker == 0 {
            return None;
        }
        castle.life = castle.life.wrapping_sub(damage);
        castle.pending_attacker_entity_index = 0;
        let lethal = castle.life < 0;
        if lethal {
            castle.killer_entity_index = attacker;
        } else {
            castle.pending_damage = 0;
            // sub_609E0 clears the damage mailbox, marks the owner's linked
            // player record, and requests state-0 reconstruction when the
            // attacker mailbox points back at this Castle root.
            if attacker == castle.source_id_entity_index && castle.level < 7 {
                castle.control_flags |= 0x40;
            }
        }
        if !lethal && castle.owner_entity_index == local_owner {
            self.player_castle_aux_flags = 4;
        }
        self.events.push(Level2EconomyEvent::DamageApplied {
            target_entity_index: target_slot,
            attacker_entity_index: attacker,
            damage,
            lethal,
        });
        Some(lethal)
    }
}

#[cfg(test)]
#[path = "level2_dynamic_economy_tests/mod.rs"]
mod tests;
