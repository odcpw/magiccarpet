use super::*;

/// Current fixed-address payload read by `AddBallon_60AB0`. This is a
/// callback-local view, not a generation-bound target handle.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Level2CastleCollectorTarget {
    pub(crate) source_slot: u16,
    pub(crate) source_class: u8,
    pub(crate) source_model: u8,
    pub(crate) position: Position,
    pub(crate) player_entity_index: u16,
    pub(crate) mana: i32,
    pub(crate) source_dword_10: i32,
    pub(crate) collision_yaw: i16,
    pub(crate) collision_pitch: i16,
    pub(crate) collision_roll: i16,
    pub(crate) collision_fov: i16,
}

pub(crate) trait Level2CastleCollectorConsumer {
    fn read(&self, economy: &Level2EconomyState, slot: u16) -> Option<Level2CastleCollectorTarget>;
    fn set_attraction(&mut self, economy: &mut Level2EconomyState, slot: u16, in_range: bool, holder: u16);
    fn add_mana(&mut self, economy: &mut Level2EconomyState, slot: u16, delta: i32);
    fn hide(&mut self, economy: &mut Level2EconomyState, slot: u16);
    fn record_damage(&mut self);
}

fn level2_castle_collector_target_overlap(defender: Position, target: &Level2CastleCollectorTarget) -> bool {
    let dx = i32::from(target.position.x.wrapping_sub(defender.x).cast_signed());
    let dy = i32::from(target.position.y.wrapping_sub(defender.y).cast_signed());
    let dz = i32::from(target.position.z) + i32::from(target.collision_yaw)
        - i32::from(defender.z) - i32::from(LEVEL2_CASTLE_DEFENDER_COLLISION_VERTICAL_OFFSET);
    dx.abs() < i32::from(target.collision_pitch) + i32::from(LEVEL2_CASTLE_DEFENDER_COLLISION_HALF_WIDTH)
        && dy.abs() < i32::from(target.collision_roll) + i32::from(LEVEL2_CASTLE_DEFENDER_COLLISION_HALF_WIDTH)
        && dz.abs() < i32::from(target.collision_fov) + i32::from(LEVEL2_CASTLE_DEFENDER_COLLISION_VERTICAL_OFFSET)
}

#[cfg(test)]
struct EconomyCollectorTestConsumer;

#[cfg(test)]
impl Level2CastleCollectorConsumer for EconomyCollectorTestConsumer {
    fn read(&self, economy: &Level2EconomyState, slot: u16) -> Option<Level2CastleCollectorTarget> {
        if let Some(row) = economy.spheres.iter().find(|row| row.active && row.source_entity_index == slot) {
            return Some(Level2CastleCollectorTarget {
                source_slot: slot, source_class: 10, source_model: 39, position: row.position,
                player_entity_index: row.player_entity_index, mana: row.mana, source_dword_10: 0,
                collision_yaw: row.collision_yaw, collision_pitch: row.collision_pitch,
                collision_roll: row.collision_roll, collision_fov: row.collision_fov,
            });
        }
        economy.castle_at(slot).map(|row| Level2CastleCollectorTarget {
            source_slot: slot, source_class: 3, source_model: 2, position: row.position,
            player_entity_index: row.player_entity_index, mana: row.mana,
            source_dword_10: i32::from(row.level),
            collision_yaw: 0, collision_pitch: 0, collision_roll: 0, collision_fov: 0,
        })
    }
    fn set_attraction(&mut self, economy: &mut Level2EconomyState, slot: u16, in_range: bool, holder: u16) {
        if let Some(row) = economy.spheres.iter_mut().find(|row| row.active && row.source_entity_index == slot) {
            if in_range { row.control_flags |= 0x40; row.assigned_defender_slot = holder; }
            else { row.control_flags &= !0x40; }
        }
    }
    fn add_mana(&mut self, economy: &mut Level2EconomyState, slot: u16, delta: i32) {
        if let Some(row) = economy.castle_at_mut(slot) { row.mana = row.mana.wrapping_add(delta); }
    }
    fn hide(&mut self, economy: &mut Level2EconomyState, slot: u16) {
        if let Some(row) = economy.spheres.iter_mut().find(|row| row.active && row.source_entity_index == slot) {
            row.pending_delete = true;
            row.control_flags |= ENTITY_DRAW_DISABLED_BIT;
        }
    }
    fn record_damage(&mut self) {}
}

/// Effects of one 66B30/66D00 visit. The caller owns the caster's actual
/// holder relation and applies a requested clear in the same source visit.
pub(crate) struct CastleProjectileVisit {
    pub(crate) live: bool,
    pub(crate) clear_holder_owner: Option<u16>,
}

impl Level2OrdinaryResetSession {
    /// `sub_11960` runs before the construction-site refusal. Only the
    /// frozen source model45 list is eligible; ordinary destruction/reclaim
    /// remains the building owner's following scheduled work.
    pub(crate) fn begin_castle_construction_source(
        &mut self,
        arena: &mut crate::campaign::source_arena::SourceArena,
        castle_slot: u16,
        model45_slots: &[u16],
    ) -> std::result::Result<usize, String> {
        let Some(castle) = arena.economy.castle_at(castle_slot).copied()
            .filter(|root| root.action == 5 && root.build_state == 0)
        else { return Ok(0) };
        let (pitch, roll) = level2_castle_shift_axes(castle.level.wrapping_add(1));
        let mut killed = 0;
        for &slot in model45_slots {
            let context = super::source_target::Context { houses: Some(&self.houses), ..Default::default() };
            let Some(target) = super::source_target::read(arena, context, slot)? else { continue };
            if (target.class, target.model) != (10, 45) { continue; }
            let Some(shape) = target.collision else {
                return Err(format!("Castle constructor model45 slot {slot} lacks source shifts"));
            };
            let dx = i32::from(target.position.x.wrapping_sub(castle.position.x) as i16).abs();
            let dy = i32::from(target.position.y.wrapping_sub(castle.position.y) as i16).abs();
            if dx > i32::from(shape.half_width) + i32::from(pitch)
                || dy > i32::from(shape.half_depth) + i32::from(roll) { continue; }
            if let Some(house) = self.houses.house_mut(slot).filter(|house| house.source_retained()) {
                house.life = -1;
                house.parameter.destruction_successor = 0;
            } else if let Some(building) = arena.economy.authored_model45.iter_mut()
                .find(|row| row.source_entity_index == slot && row.active)
            {
                building.life = -1;
                building.font_type_index = 0;
            } else if let Some(raw) = arena.entities.get_mut(usize::from(slot))
                .filter(|row| row.class == 10 && row.model == 45)
            {
                raw.set_life(-1);
                raw.font_type_index = 0;
                raw.record[65] = 0;
            } else {
                return Err(format!("Castle constructor lost model45 slot {slot}"));
            }
            killed += 1;
        }
        arena.economy.restore_castle_site_shifts(castle_slot);
        Ok(killed)
    }
}

impl crate::campaign::source_arena::SourceArena {
    /// Resolve the actual fixed-address actor mailbox alongside the typed
    /// house and mana owners visited by the same Possession pulse.
    pub(super) fn step_possession_effect_source(
        &mut self,
        source_slot: u16,
        houses: &mut HouseFamily,
    ) -> bool {
        self.economy.step_possession_effect_slot_with_entities(
            source_slot,
            houses,
            Some(&mut self.entities),
        )
    }

    /// Terminal NewEvent competes with raw and typed arena occupants through
    /// the same allocator as the ordinary source constructors.
    pub(super) fn step_possession_projectile_source(
        &mut self,
        terrain: &mut SessionTerrain,
        houses: &[HouseState],
        house_slots: &[u16],
        slot: u16,
    ) -> Level2PossessionStep {
        self.step_possession_projectile_source_inner(terrain, houses, house_slots, slot, None)
    }

    pub(super) fn step_possession_projectile_source_with_frame(
        &mut self,
        terrain: &mut SessionTerrain,
        houses: &[HouseState],
        house_slots: &[u16],
        slot: u16,
        frame: &super::source_target::PossessionCollisionFrame,
    ) -> Level2PossessionStep {
        self.step_possession_projectile_source_inner(
            terrain,
            houses,
            house_slots,
            slot,
            Some(frame),
        )
    }

    fn step_possession_projectile_source_inner(
        &mut self,
        terrain: &mut SessionTerrain,
        houses: &[HouseState],
        house_slots: &[u16],
        slot: u16,
        frame: Option<&super::source_target::PossessionCollisionFrame>,
    ) -> Level2PossessionStep {
        let mut step = self.economy.advance_possession_projectile_slot(
            slot,
            terrain,
            houses,
            house_slots,
            &self.entities,
            true,
            frame,
        );
        if let Some(target) = step.pending_tiered_contact {
            let projectile = self
                .economy
                .possession_projectiles
                .iter()
                .find(|row| row.source_entity_index == slot)
                .copied()
                .expect("contacting Possession flyer remains until next source boundary");
            let target_id = |slot: u16| {
                self.entities
                    .get(usize::from(slot))
                    .filter(|row| slot != 0 && row.active)
                    .map(|row| row.source_id)
                    .or_else(|| {
                        self.economy
                            .spheres
                            .iter()
                            .find(|row| row.active && row.source_entity_index == slot)
                            .map(|row| row.source_entity_index)
                    })
                    .or_else(|| {
                        houses
                            .iter()
                            .find(|row| row.entity_index == slot)
                            .map(|row| row.source_id)
                    })
            };
            let hit = frame
                .map_or_else(
                    || target_id(target).zip(target_id(projectile.target_source_entity_index)),
                    |frame| {
                        frame
                            .targets
                            .iter()
                            .find(|row| row.slot == target)
                            .map(|row| row.source_id)
                            .zip(frame.guidance_source_id)
                    },
                )
                .is_some_and(|(hit, prior)| hit == prior);
            if let Some(child) = self.allocate_source_slot_with_terrain(terrain) {
                let model = if projectile.output_model == 69 {
                    70
                } else {
                    12
                };
                if self
                    .economy
                    .initialize_possession_effect_model(child, projectile, model)
                    .is_some()
                {
                    crate::world::effects::sync_position(
                        self,
                        terrain,
                        child,
                        projectile.position,
                        true,
                        false,
                    )
                    .expect("allocated Possession contact effect has a valid fixed map slot");
                    step.accuracy = Some((projectile.source_id_entity_index, hit));
                }
            }
            if matches!(projectile.output_model, 0 | 54 | 69)
                && let Some(child) = self.allocate_source_slot_with_terrain(terrain)
            {
                let born = if projectile.output_model == 0 {
                    let result = self.economy.initialize_class10_model0_effect(
                        child,
                        0,
                        projectile.source_id_entity_index,
                        projectile.yaw,
                        projectile.pitch,
                        projectile.position,
                        400,
                        0,
                        0x0002_0000,
                        128,
                        128,
                    );
                    if let Some(row) = self
                        .economy
                        .model17_spawned_children
                        .iter_mut()
                        .find(|row| row.source_entity_index == child)
                    {
                        row.dword_0x10 = projectile.source_dword_10;
                    }
                    if result.is_some() {
                        crate::world::effects::sync_position(
                            self,
                            terrain,
                            child,
                            projectile.position,
                            true,
                            false,
                        )
                        .expect("allocated Possession model0 effect has a valid fixed map slot");
                    }
                    result.is_some()
                } else {
                    self.economy
                        .initialize_possession_attraction(child, projectile);
                    true
                };
                if born {
                    step.secondary_accuracy = Some((projectile.source_id_entity_index, hit));
                    step.direct_hit_owner = Some(projectile.source_id_entity_index);
                }
            }
            step.pending_tiered_contact = None;
            return step;
        }
        if !step.pending_basic {
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
            return step;
        };
        {
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
            let hit = frame
                .map_or_else(
                    || {
                        step.pending_contact
                            .and_then(id)
                            .zip(id(projectile.target_source_entity_index))
                    },
                    |frame| {
                        step.pending_contact
                            .and_then(|slot| frame.targets.iter().find(|row| row.slot == slot))
                            .map(|row| row.source_id)
                            .zip(frame.guidance_source_id)
                    },
                )
                .is_some_and(|(hit, prior)| hit == prior);
            step.accuracy = Some((projectile.source_id_entity_index, hit));
            if step.pending_contact.is_some() {
                step.direct_hit_owner = Some(projectile.source_id_entity_index);
            }
        }
        if let Some(flyer) = self
            .economy
            .possession_projectiles
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
        {
            flyer.active = false;
        }
        crate::world::effects::sync_position(
            self,
            terrain,
            child,
            projectile.position,
            true,
            false,
        )
        .expect("allocated Possession terminal effect has a valid fixed map slot");
        step.live = false;
        step.pending_basic = false;
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
    /// Its only differences are model28 and action29 for table life >= 2.
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
        castle.timer = 30;
        castle.pending_damage = castle.pending_damage.wrapping_add(i32::from(payload));
        castle.pending_attacker_entity_index = owner;
        true
    }

    /// Whether a typed carrier currently owns this numeric source slot.
    pub(crate) fn has_live_typed_slot(&self, slot: u16) -> bool {
        self.live_arena_cursor_row(slot).is_some()
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
        2 => 69,
        _ => 0,
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

/// Current records reached from the two frozen source heads at a flyer visit.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CastleSiteRow {
    pub model: u8,
    pub position: Position,
    pub pitch: i16,
    pub roll: i16,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CastleSiteRows {
    pub class3: Vec<CastleSiteRow>,
    pub model45: Vec<CastleSiteRow>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CastleUpgradeVisit {
    pub live: bool,
    pub hit_owner: Option<u16>,
    pub clear_holder_owner: Option<u16>,
}

fn castle_site_row_nearby(row: &CastleSiteRow, position: Position) -> bool {
    let dx = i32::from(row.position.x.wrapping_sub(position.x).cast_signed().unsigned_abs());
    let dy = i32::from(row.position.y.wrapping_sub(position.y).cast_signed().unsigned_abs());
    dx <= i32::from(row.pitch) + 2_560 && dy <= i32::from(row.roll) + 2_560
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
    pub stage_mode: u8,
    pub parent_entity_index: u16,
    pub top_position: Position,
}

impl Level2FireballGuidanceTarget {
    /// sub_67CB0 preserves birth identity when Alliance changes allegiance.
    pub(super) fn can_be_acquired_by(self, owner: u16) -> bool {
        self.source_id != owner
            && (self.class != 5 || self.stage_mode != 14 || self.parent_entity_index != owner)
    }
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
pub(crate) struct Level2FireballCollisionTarget {
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
    pub accuracy: Option<(u16, bool)>,
    pub captured_holder: Option<u16>,
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
    pending_contact: Option<u16>,
    pending_tiered_contact: Option<u16>,
    pub direct_hit_owner: Option<u16>,
    pub accuracy: Option<(u16, bool)>,
    pub secondary_accuracy: Option<(u16, bool)>,
}

impl Level2PossessionStep {
    const fn live(live: bool) -> Self {
        Self {
            live,
            pending_basic: false,
            pending_contact: None,
            pending_tiered_contact: None,
            direct_hit_owner: None,
            accuracy: None,
            secondary_accuracy: None,
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
        self.spawn_possession_projectile_with_source_payload(
            launch_position,
            source_id_entity_index,
            holder_entity_index,
            yaw,
            pitch,
            player_speed,
            axis_position,
            physical_tier.min(2),
            0,
            0,
            crate::level_definition::POSSESSION_PROPERTIES,
        )
    }

    /// Source row life chooses the projectile and selector independently of
    /// the physical tier; the subspell scalar becomes the auxiliary radius.
    pub fn spawn_possession_projectile_with_source_payload(
        &mut self,
        launch_position: Position,
        source_id_entity_index: u16,
        holder_entity_index: u16,
        yaw: i16,
        pitch: i16,
        player_speed: i16,
        axis_position: Position,
        source_life: u8,
        source_subspell_index: u16,
        source_mana: i32,
        properties: crate::level_definition::PossessionProperties,
    ) -> Option<u16> {
        if source_life > 3 {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        self.initialize_possession_projectile_with_source_payload(
            source_entity_index,
            launch_position,
            source_id_entity_index,
            holder_entity_index,
            yaw,
            pitch,
            player_speed,
            axis_position,
            source_life,
            source_subspell_index,
            source_mana,
            properties,
        )
    }

    /// Initialize the family constructor in the SourceArena's allocated
    /// address, after the caller has resolved any replaced parent record.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn initialize_possession_projectile_with_source_payload(
        &mut self,
        source_entity_index: u16,
        launch_position: Position,
        source_id_entity_index: u16,
        holder_entity_index: u16,
        yaw: i16,
        pitch: i16,
        player_speed: i16,
        axis_position: Position,
        source_life: u8,
        source_subspell_index: u16,
        source_mana: i32,
        properties: crate::level_definition::PossessionProperties,
    ) -> Option<u16> {
        debug_assert!(source_life <= 3);
        let tiered = source_life != 0;
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
            output_model: possession_source_byte_0x44(source_life),
            source_mana,
            source_dword_10: if tiered {
                i32::from(source_subspell_index)
                    .wrapping_shl(8)
                    .wrapping_mul(i32::from(source_subspell_index).wrapping_shl(8))
            } else {
                200
            },
            target_source_entity_index: 0,
            acquisition_attempted: false,
            threat_accounted: false,
            position: launch_position,
            axis_position,
            yaw: yaw & 0x07ff,
            pitch: pitch & 0x07ff,
            roll: 0,
            fov: 0,
            actual_speed: properties.projectile_speed.wrapping_add(player_speed),
            minimum_speed: properties.projectile_speed,
            life: properties.projectile_lifetime,
            max_life: properties.projectile_lifetime,
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
        self.initialize_fireball_projectile_with_properties(
            source_entity_index,
            payload,
            player_speed,
            0,
            crate::level_definition::FIREBALL_PROPERTIES,
        )
    }

    pub(super) fn initialize_fireball_projectile_with_properties(
        &mut self,
        source_entity_index: u16,
        payload: Level2FireballProjectileLaunchPayload,
        player_speed: i16,
        source_life: u8,
        properties: crate::level_definition::FireballProperties,
    ) -> Option<u16> {
        let entity_rng = self.new_event_rng(source_entity_index);
        let projectile = Level2FireballProjectile {
            origin: payload.origin,
            visual_index: properties.render_index,
            source_entity_index,
            entity_rng,
            owner_entity_index: payload.source_id_entity_index,
            holder_entity_index: payload.holder_entity_index,
            source_subspell_index: payload.source_subspell_index,
            source_mana: payload.source_mana,
            source_class: FIREBALL_PROJECTILE_CLASS,
            source_model: if source_life >= 2 {
                28
            } else {
                FIREBALL_PROJECTILE_MODEL
            },
            action: if source_life >= 2 { 29 } else { 0 },
            payload: i32::from(payload.source_subspell_index),
            position: payload.position,
            axis_position: payload.axis_position,
            yaw: payload.yaw & 0x07ff,
            pitch: payload.pitch & 0x07ff,
            roll: payload.yaw & 0x07ff,
            fov: payload.pitch & 0x07ff,
            actual_speed: properties
                .projectile_speed
                .wrapping_add(player_speed)
                .clamp(FIREBALL_SPEED, 0x2000),
            minimum_speed: properties.projectile_speed,
            life: properties.projectile_lifetime,
            max_life: properties.projectile_lifetime,
            dword_0x10: payload.source_dword_0x10,
            target_source_entity_index: payload.target_source_entity_index,
            target_class: payload.target_class,
            target_model: payload.target_model,
            acquisition_attempted: !matches!(
                payload.origin,
                FireballProjectileOrigin::PlayerSpell
                    | FireballProjectileOrigin::Firefly
                    | FireballProjectileOrigin::Dragon
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
            .filter(|target| target.index != 0 && target.class == 3 && target.model == 0)
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
            false,
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
        initial_capture: bool,
    ) -> std::result::Result<Level2FireballStep, String> {
        let guidance_targets = self.fireball_guidance_targets(imported_entities)?;
        let collision_targets = if views.is_none() {
            self.fireball_collision_targets(imported_entities)
        } else {
            Vec::new()
        };
        let Some(projectile_index) = self.fireball_projectiles.iter().position(|projectile| {
            projectile.active && projectile.source_entity_index == source_slot
        }) else {
            return Ok(Level2FireballStep {
                captured_holder: None,
                accuracy: None,
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
        let source_fireball =
            projectile.source_class == 9 && matches!(projectile.source_model, 0 | 28);

        if matches!(
            projectile.origin,
            FireballProjectileOrigin::Firefly
                | FireballProjectileOrigin::Dragon
                | FireballProjectileOrigin::Worm
        ) && projectile.target_source_entity_index != 0
        {
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
                imported_entities
                    .get(usize::from(projectile.target_source_entity_index))
                    .filter(|target| {
                        target.active
                            && target.life >= 0
                            && target.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
                    })
                    .map(|target| target.position)
            };
            if let Some(position) = target_position {
                let (_, desired_pitch) = projectile_guidance_angles(projectile.position, position);
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
                {
                    let mut target = target;
                    if let Some(views) = views {
                        let Some(current) = views
                            .get(usize::from(target.source_slot))
                            .ok_or("Fireball acquisition address is outside arena")?
                            .as_ref()
                            .map_err(Clone::clone)?
                        else {
                            continue;
                        };
                        if current.class == 3 && current.control_flags & 0x20 != 0 {
                            continue;
                        }
                        target.source_id = current.source_id;
                        target.class = current.class;
                        target.model = current.model;
                        target.stage_mode = current.relations.stage_mode.unwrap_or(0);
                        target.parent_entity_index = current.relations.parent.unwrap_or(0);
                        target.top_position = if current.model == 2 {
                            current.position
                        } else {
                            let shape = current.collision.ok_or_else(|| {
                                format!(
                                    "Fireball acquisition target {} lacks current source axes",
                                    target.source_slot
                                )
                            })?;
                            super::guided_projectile::centre(
                                current.position,
                                current.model,
                                shape.vertical_offset,
                            )
                        };
                    }
                    if !target.can_be_acquired_by(projectile.owner_entity_index)
                        || !fireball_target_selector_matches(
                            &projectile,
                            target.class,
                            target.model,
                        )
                    {
                        continue;
                    }
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
        } else if source_fireball {
            let position = if let Some(views) = views {
                match views
                    .get(usize::from(projectile.target_source_entity_index))
                    .ok_or("Fireball target address is outside arena")?
                    .as_ref()
                    .map_err(Clone::clone)?
                {
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
                imported_entities
                    .get(usize::from(projectile.target_source_entity_index))
                    .map(|row| {
                        super::guided_projectile::centre(row.position, row.model, row.array_yaw())
                    })
            };
            if let Some(position) = position {
                (projectile.roll, projectile.fov) =
                    projectile_guidance_angles(projectile.position, position);
                projectile.yaw = projectile.yaw.wrapping_add(turn_toward(
                    projectile.yaw,
                    projectile.roll,
                    if initial_capture {
                        FIREBALL_INITIAL_YAW_TURN_LIMIT
                    } else {
                        FIREBALL_HOMING_YAW_TURN_LIMIT
                    },
                )) & 0x07ff;
                projectile.pitch = if initial_capture {
                    projectile.fov
                } else {
                    projectile.pitch.wrapping_add(turn_toward(
                        projectile.pitch,
                        projectile.fov,
                        FIREBALL_HOMING_PITCH_TURN_LIMIT,
                    )) & 0x07ff
                };
                if initial_capture {
                    projectile.acquisition_attempted = true;
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

        // sub_65C20 (model0/28) retains inherited speed. The distinct
        // sub_65820 routes ease speed before movement.
        if !source_fireball {
            let speed_delta =
                i32::from(projectile.minimum_speed) - i32::from(projectile.actual_speed);
            projectile.actual_speed = projectile
                .actual_speed
                .wrapping_add((2 * speed_delta.signum()) as i16);
        }
        let previous_position = projectile.position;
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
                    captured_holder: None,
                    accuracy: None,
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
                // sub_65C20 restores the pre-move XY; sub_65820 retains moved XY.
                if source_fireball {
                    projectile.position = previous_position;
                }
                projectile.position.z = height;
                if terrain.material_mask(projectile.position) == 1 {
                    if defer_birth {
                        self.fireball_projectiles[projectile_index] = projectile;
                        return Ok(Level2FireballStep {
                            captured_holder: None,
                            accuracy: None,
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
                        captured_holder: None,
                        accuracy: None,
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
                captured_holder: None,
                accuracy: None,
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
                captured_holder: None,
                accuracy: None,
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
            captured_holder: None,
            accuracy: None,
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
    ) -> std::result::Result<Vec<Level2FireballGuidanceTarget>, String> {
        let class3_slots = imported_entities
            .iter()
            .filter(|entity| {
                entity.index != 0
                    && entity.active
                    && entity.life >= 0
                    && entity.class == 3
                    && entity.control_flags & 0x20 == 0
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
                        stage_mode: entity.record[77],
                        parent_entity_index: entity.source_spell_parent_entity_index(),
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
                        stage_mode: entity.record[77],
                        parent_entity_index: entity.source_spell_parent_entity_index(),
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
                            stage_mode: bee.stage_var2,
                            parent_entity_index: bee.parent_entity_index,
                            top_position: bee.position,
                        })
                }));
            }
            if matches!(
                model,
                level2_class5::archer::ARCHER_MODEL | level2_class5::firefly::FIREFLY_MODEL
            ) {
                for source in self.authored_class5.iter().filter(|source| {
                    source.active
                        && source.drawing_enabled
                        && source.life >= 0
                        && source.model == model
                        && source.source_byte_39 != 0
                }) {
                    let (source_id, stage_mode, parent_entity_index) =
                        if model == level2_class5::archer::ARCHER_MODEL {
                            let state = self
                                .authored_class5_archer
                                .iter()
                                .find(|state| state.source_entity_index == source.source_entity_index)
                                .ok_or_else(|| {
                                    format!(
                                        "Fireball target {} lost its Archer state",
                                        source.source_entity_index,
                                    )
                                })?;
                            (state.entity_id, state.stage.mode, state.stage.parent)
                        } else {
                            let state = self
                                .authored_fireflies
                                .iter()
                                .find(|state| state.source_entity_index == source.source_entity_index)
                                .ok_or_else(|| {
                                    format!(
                                        "Fireball target {} lost its Firefly state",
                                        source.source_entity_index,
                                    )
                                })?;
                            (state.entity_id, state.stage_var2, state.parent_entity_index)
                        };
                    bucket.push(Level2FireballGuidanceTarget {
                        source_slot: source.source_entity_index,
                        source_id,
                        class: 5,
                        model: source.model,
                        stage_mode,
                        parent_entity_index,
                        top_position: source.position,
                    });
                }
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
                        source_id: render.entity_id,
                        class: 5,
                        model: source.model,
                        stage_mode: render.stage_var2,
                        parent_entity_index: render.parent_entity_index,
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
                        // sub_4B240 copies the root's constructor StageVar2;
                        // the retained part carrier has no stage callback.
                        stage_mode: 0,
                        parent_entity_index: part.parent_entity_index,
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
        Ok(targets)
    }

    /// Project the map-linked rows needed by `sub_10780`. The qualified
    /// Level-2 route exercises imported class-3 rows, typed Bee rows, and the
    /// typed Castle root; all retain the source low-byte collision flag and
    /// fixed slot identity used by the map chain.
    pub(super) fn fireball_collision_targets(
        &self,
        imported_entities: &[EntitySnapshot],
    ) -> Vec<Level2FireballCollisionTarget> {
        self.projectile_collision_targets(imported_entities, false)
    }

    /// sub_10780 follows current linked addresses even while hidden or dead;
    /// the arrow callback applies the source bit8 and selector predicates.
    pub(super) fn arrow_collision_targets(
        &self,
        imported_entities: &[EntitySnapshot],
    ) -> Vec<Level2FireballCollisionTarget> {
        self.projectile_collision_targets(imported_entities, true)
    }

    fn projectile_collision_targets(
        &self,
        imported_entities: &[EntitySnapshot],
        retain_linked: bool,
    ) -> Vec<Level2FireballCollisionTarget> {
        let mut targets = imported_entities
            .iter()
            .filter(|entity| {
                entity.index != 0
                    && entity.active
                    && (retain_linked || entity.life >= 0 && entity.control_flags & ENTITY_DRAW_DISABLED_BIT == 0)
                    && (!retain_linked || self.live_arena_kind(entity.index).is_none())
                    && entity.control_flags & 8 != 0
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
                && (retain_linked || bee.drawing_enabled && bee.life >= 0)
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
                && (retain_linked || actor.drawing_enabled() && actor.life >= 0)
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
                && (retain_linked || source.drawing_enabled && source.life >= 0)
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
                && (retain_linked || source.drawing_enabled && source.life >= 0
                    && render.control_flags & ENTITY_DRAW_DISABLED_BIT == 0)
                && source.model == level2_class5::dragon::DRAGON_MODEL
                && render.control_flags & 8 != 0
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
                && (retain_linked || part.drawing_enabled && part.life >= 0)
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
            (retain_linked || castle.life >= 0 && castle.control_flags & ENTITY_DRAW_DISABLED_BIT == 0)
                && castle.control_flags & 8 != 0
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
        if retain_linked {
            targets.extend(self.authored_class5.iter().filter_map(|source| {
                let state = self.authored_class5_model3.iter()
                    .find(|state| state.source_entity_index == source.source_entity_index)?;
                (source.active && source.model == 3 && state.control_flags & 8 != 0
                    && self.map_chains.is_linked(source.source_entity_index))
                    .then_some(Level2FireballCollisionTarget {
                        source_slot: source.source_entity_index, source_id: state.entity_id,
                        class: 5, model: 3, position: source.position,
                        half_width: state.render_pitch, vertical_offset: state.render_yaw,
                        half_height: state.render_fov,
                    })
            }));
            targets.extend(self.authored_class5_model3_children.iter().filter_map(|part| {
                (part.active && part.control_flags & 8 != 0 && self.map_chains.is_linked(part.source_entity_index))
                    .then_some(Level2FireballCollisionTarget {
                        source_slot: part.source_entity_index, source_id: part.parent_entity_index,
                        class: 5, model: part.model, position: part.position,
                        half_width: part.render_pitch, vertical_offset: part.render_yaw,
                        half_height: part.render_fov,
                    })
            }));
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
        self.initialize_possession_effect_model(
            source_entity_index,
            projectile,
            projectile.output_model,
        )
    }

    fn initialize_possession_effect_model(
        &mut self,
        source_entity_index: u16,
        projectile: Level2PossessionProjectile,
        model: u8,
    ) -> Option<u16> {
        let entity_rng = self.new_event_rng(source_entity_index);
        let effect = Level2PossessionEffect {
            source_entity_index,
            entity_rng,
            source_id_entity_index: projectile.source_id_entity_index,
            projectile_entity_index: projectile.source_entity_index,
            source_class: projectile.output_class,
            source_model: model,
            action: if model == 70 {
                77
            } else {
                POSSESSION_IMPACT_ACTION
            },
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
            None,
        )
    }

    /// Compute the source callback's guidance, movement and initial terrain
    /// clamp without mutating the row. Collision-frame construction uses the
    /// same calculation so it visits only the cells the callback will scan.
    pub(crate) fn possession_projectile_moved(
        &self,
        source_slot: u16,
        terrain: &SessionTerrain,
        houses: &[HouseState],
        house_guidance_slots: &[u16],
        entities: &[EntitySnapshot],
        current_guidance: Option<Option<Position>>,
        acquisition_candidates: Option<&[(u16, Position)]>,
    ) -> Option<Level2PossessionProjectile> {
        let mut moved = *self.possession_projectiles.iter().find(|projectile| {
            projectile.active && projectile.source_entity_index == source_slot
        })?;

        // `CastPosses_65F60` resolves word_0x96 on entry. A sentinel target
        // permits exactly one `sub_67CB0` attempt; success copies roll/fov
        // directly into yaw/pitch before movement. A pre-existing relation
        // instead runs `sub_65610`, refreshing desired angles and capping both
        // turns by descriptor-2's 0x71 source limit.
        if moved.target_source_entity_index == 0 {
            if !moved.acquisition_attempted {
                moved.acquisition_attempted = true;
                let target = acquisition_candidates.map_or_else(
                    || level2_possession_guidance_target(
                        &self.spheres,
                        &self.possession_guidance_source_slots,
                        houses,
                        house_guidance_slots,
                        entities,
                        moved,
                    ),
                    |candidates| {
                        let mut best = None;
                        let mut best_score = u32::MAX;
                        for &(slot, top) in candidates {
                            if let Some(score) =
                                projectile_target_score(moved.position, moved.yaw, moved.pitch, top)
                                && score < best_score
                            {
                                best = Some((slot, top));
                                best_score = score;
                            }
                        }
                        best
                    },
                );
                if let Some((target_source_entity_index, target_top)) = target {
                    moved.target_source_entity_index = target_source_entity_index;
                    (moved.roll, moved.fov) =
                        projectile_guidance_angles(moved.position, target_top);
                    moved.yaw = moved.roll;
                    moved.pitch = moved.fov;
                }
            }
        } else if let Some(target_top) = current_guidance.unwrap_or_else(|| level2_possession_guidance_relation(
            &self.spheres,
            houses,
            &self.possession_projectiles,
            &self.possession_guidance_stale_records,
            moved.target_source_entity_index,
        )) {
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
        crate::move_entity(
            &mut moved.position,
            moved.yaw,
            moved.pitch,
            moved.actual_speed,
        );
        moved.phase = moved.phase.wrapping_add(1);
        let ground = terrain.ground_height(moved.position);
        if ground > moved.position.z {
            moved.position.z = ground;
        }
        if moved.source_model != POSSESSION_TIERED_PROJECTILE_MODEL
            && let Some(ceiling) = terrain.ceiling_height(moved.position)
        {
            let limit = ceiling.wrapping_sub(POSSESSION_PROJECTILE_COLLISION_HALF_HEIGHT);
            if limit < moved.position.z {
                moved.position.z = limit;
            }
        }
        Some(moved)
    }

    fn advance_possession_projectile_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
        houses: &[HouseState],
        house_guidance_slots: &[u16],
        entities: &[EntitySnapshot],
        defer_birth: bool,
        frame: Option<&super::source_target::PossessionCollisionFrame>,
    ) -> Level2PossessionStep {
        let Some(projectile_index) = self.possession_projectiles.iter().position(|projectile| {
            projectile.active && projectile.source_entity_index == source_slot
        }) else {
            return Level2PossessionStep::live(false);
        };
        let Some(mut moved) = self.possession_projectile_moved(
            source_slot,
            terrain,
            houses,
            house_guidance_slots,
            entities,
            frame.map(|frame| frame.guidance),
            frame.map(|frame| frame.acquisition_candidates.as_slice()),
        ) else {
            return Level2PossessionStep::live(false);
        };

        if moved.source_model == POSSESSION_TIERED_PROJECTILE_MODEL {
            return self.finish_tiered_possession_move(
                projectile_index,
                moved,
                terrain,
                houses,
                frame,
            );
        }

        // `CastPosses_65F60` strictly clamps the flyer before its `sub_108B0`
        // scan. These constraints only choose the scan position: an ordinary
        // initial clamp is not a model-12 impact and equality is not contact.
        let ground_height = terrain.ground_height(moved.position);
        let cave_limit = terrain
            .ceiling_height(moved.position)
            .map(|ceiling_height| {
                ceiling_height.wrapping_sub(POSSESSION_PROJECTILE_COLLISION_HALF_HEIGHT)
            });

        let target = self.possession_contact_target(moved, houses, frame);
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

    fn possession_contact_target(
        &self,
        projectile: Level2PossessionProjectile,
        houses: &[HouseState],
        frame: Option<&super::source_target::PossessionCollisionFrame>,
    ) -> Option<(u16, Position)> {
        let Some(frame) = frame else {
            return level2_first_possession_target(
                &self.map_chains,
                &self.spheres,
                houses,
                projectile,
            );
        };
        let half_height = if projectile.source_model == POSSESSION_TIERED_PROJECTILE_MODEL {
            2 * POSSESSION_PROJECTILE_COLLISION_VERTICAL_OFFSET
        } else {
            POSSESSION_PROJECTILE_COLLISION_HALF_HEIGHT
        };
        for slot in source_map_search_slots(
            &self.map_chains,
            projectile.position,
            &LEVEL2_POSSESSION_PROJECTILE_SEARCH_OFFSETS,
        ) {
            let Some(target) = frame.targets.iter().find(|target| target.slot == slot) else {
                continue;
            };
            let shape = target.shape;
            let dx = i32::from(
                projectile
                    .position
                    .x
                    .wrapping_sub(target.position.x)
                    .cast_signed(),
            )
            .abs();
            let dy = i32::from(
                projectile
                    .position
                    .y
                    .wrapping_sub(target.position.y)
                    .cast_signed(),
            )
            .abs();
            let dz = (i32::from(projectile.position.z)
                + i32::from(POSSESSION_PROJECTILE_COLLISION_VERTICAL_OFFSET)
                - i32::from(target.position.z)
                - i32::from(shape.vertical_offset))
            .abs();
            if dx
                < i32::from(POSSESSION_PROJECTILE_COLLISION_HALF_WIDTH)
                    + i32::from(shape.half_width)
                && dy
                    < i32::from(POSSESSION_PROJECTILE_COLLISION_HALF_WIDTH)
                        + i32::from(shape.half_depth)
                && dz < i32::from(half_height) + i32::from(shape.half_height)
            {
                return Some((
                    slot,
                    Position {
                        z: target.position.z.wrapping_add(if target.model == 2 {
                            0
                        } else {
                            shape.vertical_offset
                        }),
                        ..target.position
                    },
                ));
            }
        }
        None
    }

    /// `sub_674C0` tests a ground-clamped position before checking the cave
    /// ceiling. Only a target contact creates descendants; terminal terrain
    /// and lifetime hide the flyer without constructing anything.
    fn finish_tiered_possession_move(
        &mut self,
        projectile_index: usize,
        mut moved: Level2PossessionProjectile,
        terrain: &SessionTerrain,
        houses: &[HouseState],
        frame: Option<&super::source_target::PossessionCollisionFrame>,
    ) -> Level2PossessionStep {
        const HALF_HEIGHT: i16 = 2 * POSSESSION_PROJECTILE_COLLISION_VERTICAL_OFFSET;
        let contact = self.possession_contact_target(moved, houses, frame);
        let terminal = if let Some((_, position)) = contact {
            moved.position = position;
            true
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
                true
            } else {
                moved.life = moved.life.wrapping_sub(1);
                moved.life < 0
            }
        };
        if terminal {
            moved.active = false;
        }
        self.possession_projectiles[projectile_index] = moved;
        let _ = self.map_chains.sync_entity(RenderableMapSeed {
            entity_index: moved.source_entity_index,
            active: true,
            x: moved.position.x,
            y: moved.position.y,
        });
        Level2PossessionStep {
            pending_tiered_contact: contact.map(|(slot, _)| slot),
            ..Level2PossessionStep::live(moved.active)
        }
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
            radius_squared: projectile.source_dword_10 as u32,
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
        self.step_possession_effect_slot_with_entities(source_slot, houses, None)
    }

    fn step_possession_effect_slot_with_entities(
        &mut self,
        source_slot: u16,
        houses: &mut HouseFamily,
        mut entities: Option<&mut [EntitySnapshot]>,
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
                // Model22 is an imported fixed-address family. Its scheduled
                // callback reads this very mailbox and propagates ownership
                // through the linked creature, including child contacts.
                if self.live_arena_kind(candidate_slot).is_none()
                    && let Some(actor) = entities
                        .as_deref_mut()
                        .and_then(|rows| rows.get_mut(usize::from(candidate_slot)))
                    && actor.active
                    && matches!((actor.class, actor.model), (5, 22) | (10, 57))
                    && actor.control_flags & 8 != 0
                    && actor.record[60] & 2 != 0
                    && actor.source_id != effect.source_id_entity_index
                {
                    let axis = |offset| {
                        i16::from_le_bytes([actor.record[offset], actor.record[offset + 1]])
                    };
                    let dx = i32::from(
                        effect
                            .position
                            .x
                            .wrapping_sub(actor.position.x)
                            .cast_signed(),
                    )
                    .abs();
                    let dy = i32::from(
                        effect
                            .position
                            .y
                            .wrapping_sub(actor.position.y)
                            .cast_signed(),
                    )
                    .abs();
                    let dz = (i32::from(effect.position.z)
                        + i32::from(POSSESSION_IMPACT_COLLISION_VERTICAL_OFFSET)
                        - i32::from(actor.position.z)
                        - i32::from(axis(86)))
                    .abs();
                    if dx < i32::from(POSSESSION_IMPACT_COLLISION_HALF_WIDTH) + i32::from(axis(88))
                        && dy
                            < i32::from(POSSESSION_IMPACT_COLLISION_HALF_WIDTH)
                                + i32::from(axis(90))
                        && dz
                            < i32::from(POSSESSION_IMPACT_COLLISION_HALF_HEIGHT)
                                + i32::from(axis(92))
                    {
                        actor.record[104..108]
                            .copy_from_slice(&u32::from(effect.source_model == 70).to_le_bytes());
                        actor.record[108..110]
                            .copy_from_slice(&effect.source_id_entity_index.to_le_bytes());
                    }
                }
                if let Some(marker) = self.player_death_markers.iter_mut().find(|marker| {
                    marker.source_entity_index == candidate_slot
                        && marker.active
                        && marker.control_flags & 8 != 0
                        && marker.byte_38 & 2 != 0
                        && marker.source_entity_index != effect.source_id_entity_index
                }) && let Some(raw) = entities
                    .as_deref()
                    .and_then(|rows| rows.get(usize::from(candidate_slot)))
                {
                    let axis =
                        |offset| i16::from_le_bytes([raw.record[offset], raw.record[offset + 1]]);
                    let dx = i32::from(
                        effect
                            .position
                            .x
                            .wrapping_sub(marker.position.x)
                            .cast_signed(),
                    )
                    .abs();
                    let dy = i32::from(
                        effect
                            .position
                            .y
                            .wrapping_sub(marker.position.y)
                            .cast_signed(),
                    )
                    .abs();
                    let dz = (i32::from(effect.position.z)
                        + i32::from(POSSESSION_IMPACT_COLLISION_VERTICAL_OFFSET)
                        - i32::from(marker.position.z)
                        - i32::from(axis(86)))
                    .abs();
                    if dx < i32::from(POSSESSION_IMPACT_COLLISION_HALF_WIDTH) + i32::from(axis(88))
                        && dy
                            < i32::from(POSSESSION_IMPACT_COLLISION_HALF_WIDTH)
                                + i32::from(axis(90))
                        && dz
                            < i32::from(POSSESSION_IMPACT_COLLISION_HALF_HEIGHT)
                                + i32::from(axis(92))
                    {
                        marker.pending_possession_owner = effect.source_id_entity_index;
                        marker.pending_possession_payload = u32::from(effect.source_model == 70);
                    }
                }
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
                        .submit_possession(
                            candidate_slot,
                            effect.source_id_entity_index,
                            u32::from(effect.source_model == 70),
                        )
                        .expect("eligible live House accepts ordinary Possession mailbox");
                }
                let Some(sphere_index) = self.spheres.iter().position(|sphere| {
                    sphere.active
                        && !sphere.pending_delete
                        && sphere.control_flags & 8 != 0
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
                    sphere.incoming_possession_payload = u32::from(effect.source_model == 70);
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
            player_entity_index: 0,
            pending_possession_owner: 0,
            pending_possession_payload: 0,
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

    /// Ground an ordinary action-42 visit. The possession adapter below
    /// additionally resolves and consumes its current claimant mailbox.
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

    /// Source 36AE0 grounds first, then consumes word68. The caller resolves
    /// the possessor's current class and applies the returned physical word94
    /// transfer over the complete source arena before publishing this marker.
    pub(crate) fn step_player_death_marker_possession(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
        possessor_class: Option<u8>,
    ) -> Option<u16> {
        if !self.step_player_death_marker_slot(source_slot, terrain) {
            return None;
        }
        let marker = self
            .player_death_markers
            .iter_mut()
            .find(|marker| marker.source_entity_index == source_slot)?;
        let possessor = marker.pending_possession_owner;
        if possessor == 0 {
            return None;
        }
        let transfer =
            (possessor_class == Some(3) && marker.player_entity_index == 0).then_some(possessor);
        marker.pending_possession_owner = 0;
        marker.control_flags |= ENTITY_DRAW_DISABLED_BIT;
        transfer
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
    /// terminal no-callback branch. Model 79/action 0x56 is owned by
    /// castle_defense over the real SourceArena; model 42/action 44 is the
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
                    || (child.source_class == 10 && matches!(child.model, 41 | 42 | 43)))
        }) else {
            return false;
        };
        let child = self.castle_children[child_index];
        if child.control_flags & ENTITY_DRAW_DISABLED_BIT != 0
            && !(child.source_class == 10 && child.model == 43 && child.action == 45)
        {
            // `DisableEntityDrawing04_57F10` leaves the row for the generic
            // `UpdateEntities_57730` hidden-row prepass. That prepass runs
            // before the next dispatch walk, independently of this root.
            return true;
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

    fn step_castle_upgrade_product(&mut self, child_index: usize) -> bool {
        let child = self.castle_children[child_index];
        let current_castle_slot = self.castle_for_owner(child.id_entity_index)
            .map(|castle| castle.source_entity_index);
        let target = current_castle_slot.and_then(|slot| self.castle_at(slot)).map(|castle| {
            (castle.position, crate::combat::area::Shape {
                vertical_offset: castle.shift_yaw, half_width: castle.shift_pitch,
                half_depth: castle.shift_roll, half_height: castle.shift_fov,
            })
        });
        let visit = self.step_castle_upgrade_product_with_target(child.source_entity_index, target);
        if let Some(castle) = current_castle_slot.and_then(|slot| self.castle_entry_mut(slot)) {
            if visit.hit_owner.is_some() {
                castle.source_word_80 = child.id_entity_index;
                castle.source_word_7c = 10;
            } else if visit.clear_holder_owner.is_some() {
                castle.upgrade_holder_clear_pending = true;
            }
        }
        visit.live
    }

    /// 389F0 tests the current fixed-address occupant supplied by the arena,
    /// including the zero sentinel. Its caller applies the returned mailbox or
    /// holder update to that same current source record.
    pub(crate) fn step_castle_upgrade_product_with_target(
        &mut self,
        source_slot: u16,
        target: Option<(Position, crate::combat::area::Shape)>,
    ) -> CastleUpgradeVisit {
        let Some(child_index) = self.castle_children.iter().position(|child| {
            child.active && child.source_entity_index == source_slot
                && child.source_class == 10 && child.model == 43 && child.action == 45
        }) else {
            return CastleUpgradeVisit { live: false, hit_owner: None, clear_holder_owner: None };
        };
        let prior = self.castle_children[child_index];
        let first_visit = prior.life >= 0 && prior.control_flags & 2 == 0;
        let hit = first_visit && target.is_some_and(|(position, shape)| {
            level2_model76_sub106c0_matches(
                position, shape.vertical_offset, shape.half_width, shape.half_depth, shape.half_height,
                prior.position, prior.yaw.cast_signed(), prior.roll.cast_signed(),
                prior.roll.cast_signed(), prior.roll.cast_signed(),
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
            if first_visit {
                child.control_flags |= 0x2;
            }
            child.source_byte_3e = child.source_byte_3e.wrapping_add(1);
            child.active = true;
            *child
        };
        self.sync_castle_child_map(child);
        CastleUpgradeVisit {
            live: true,
            hit_owner: hit.then_some(child.id_entity_index),
            clear_holder_owner: (first_visit && !hit).then_some(child.id_entity_index),
        }
    }

    /// A 389F0 visit without a current Castle cannot attach its missed-hit
    /// notification to a root. The source caller still clears the caster's
    /// current holder in that same visit.
    pub(super) fn castle_upgrade_product_missing_root_owner(&self, slot: u16) -> Option<u16> {
        let child = self.castle_children.iter().find(|child| child.active
            && child.source_entity_index == slot && child.model == 43 && child.action == 45
            && child.control_flags & ENTITY_DRAW_DISABLED_BIT != 0)?;
        self.castle_for_owner(child.id_entity_index).is_none().then_some(child.id_entity_index)
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
        if sphere.owner_entity_index == owner_entity_index
            && sphere.player_entity_index == owner_entity_index
        {
            return false;
        }
        sphere.owner_entity_index = owner_entity_index;
        // The staged claim represents the result of the source possession
        // mailbox: Castle and balloon consumers read playerEntityIndex, not
        // the economy's owner projection. An ownership change also cancels
        // the previous defender's attraction, as TransformArcherToMana does.
        if sphere.player_entity_index != owner_entity_index {
            sphere.player_entity_index = owner_entity_index;
            sphere.control_flags &= !0x40;
        }
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
    #[cfg(test)]
    pub fn step_mana_sphere_slot(
        &mut self,
        sphere_slot: u16,
        terrain: &SessionTerrain,
        entities: &mut [crate::native_world::EntitySnapshot],
        color_base_for_owner: impl Fn(u16) -> u16,
    ) -> bool {
        self.step_mana_sphere_slot_with_owner_resources(sphere_slot, terrain, entities,
            color_base_for_owner, |economy, rows, slot| economy.current_owner_resources(rows, slot))
    }

    pub(crate) fn step_mana_sphere_slot_with_owner_resources(
        &mut self,
        sphere_slot: u16,
        terrain: &SessionTerrain,
        entities: &mut [EntitySnapshot],
        color_base_for_owner: impl Fn(u16) -> u16,
        owner_resources: impl Fn(&Level2EconomyState, &[EntitySnapshot], u16) -> (u8, i32),
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

        // TransformArcherToMana35940 reads the current occupant at word96. Castle3/3
        // collectors own a typed arena row with an inactive raw shadow;
        // creature5/23 instead owns its raw word2c lift step. Never let an
        // old sidecar or raw shadow override a different current occupant.
        let defender_attracted = self.spheres[sphere_index].control_flags & 0x40 != 0;
        if defender_attracted {
            let defender_slot = self.spheres[sphere_index].assigned_defender_slot;
            let current = self.live_arena_cursor_row(defender_slot);
            let holder = if defender_slot == 0 {
                None
            } else if current.is_some_and(|row| row.kind == Level2DynamicArenaKind::CastleDefender) {
                self
                    .castle_children
                    .iter()
                    .find(|child| {
                        child.active
                            && child.source_entity_index == defender_slot
                            && child.source_class == 3
                            && child.model == 3
                    })
                    .map(|child| (child.position, 32_i16))
            } else if current.is_none() {
                entities.get(usize::from(defender_slot))
                    .filter(|entity| entity.active && entity.index == defender_slot)
                    .and_then(|entity| match (entity.class, entity.model) {
                        (3, 3) => Some((entity.position, 32_i16)),
                        (5, 23) => Some((entity.position,
                            i16::from_le_bytes([entity.record[48], entity.record[49]]))),
                        _ => None,
                    })
            } else { None };
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
                self.merge_mana_spheres_with_owner_resources(sphere_slot, absorbed_slot, Some(entities), &owner_resources);
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
    #[cfg(test)]
    pub fn step_castle_defender_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
    ) -> bool {
        self.step_castle_defender_slot_with_consumer(source_slot, terrain, &mut EconomyCollectorTestConsumer)
    }

    pub(crate) fn step_castle_defender_slot_with_consumer(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
        consumer: &mut dyn Level2CastleCollectorConsumer,
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
        if child_snapshot.target_sphere_slot == 0 {
            // Source skips the movement and terrain-settle body for the null
            // sentinel, but still executes sub_60EA0's damage tail.
            self.finish_castle_defender_source_damage(child_index, consumer);
            return true;
        }
        let Some(target) = consumer.read(self, child_snapshot.target_sphere_slot) else {
            // A malformed address must not erase the actual source relation.
            self.finish_castle_defender_source_damage(child_index, consumer);
            return true;
        };
        let mut predicted = child_snapshot.position;
        let mut yaw = child_snapshot.yaw as i16;
        if target.source_class == 3 {
            yaw = i16::try_from(heading_for_positions(predicted, target.position))
                .expect("Castle defender heading fits");
            let distance = level2_wrapping_target_distance(predicted, target.position);
            let threshold = i32::from(child_snapshot.speed).wrapping_mul(target.source_dword_10) as u32;
            if distance <= threshold {
                // NewEvent_4A050 binds the holder's str_160 word_0xc to zero
                // for the default event profile.  This is the exact source
                // altitude gate before the class-3 mana transfer.
                let ground = terrain.ground_height(predicted);
                // `sub_4ABA0` binds `str_D7BD6[68].word_160_0xc_12` to 512;
                // the source compares the holder's predicted Z against that
                // terrain-relative upper bound, not against the ground
                // plane itself.
                if predicted.z <= ground.saturating_add(512) && target.source_dword_10 > 0 {
                    // The source snaps the holder's predicted axis to the
                    // class-3 target position; the target Castle itself does
                    // not move. Its mana receives the holder payload.
                    predicted.x = target.position.x;
                    predicted.y = target.position.y;
                    consumer.add_mana(self, target.source_slot, child_snapshot.mana);
                    let child = &mut self.castle_children[child_index];
                    child.mana = 0;
                    child.player_entity_index = child.id_entity_index;
                    child.life = child.max_life;
                    if target.source_model == 2 {
                        self.events.push(Level2EconomyEvent::CastleDefenderArrived {
                            defender_slot: source_slot, castle_id: target.source_slot, mana: child_snapshot.mana,
                        });
                    }
                }
            } else {
                crate::move_entity(&mut predicted, yaw, 0, child_snapshot.speed);
            }
        } else if target.source_class == 10 {
            // Source `AddBallon_60AB0` uses the defender's raw id relation for
            // the owner gate.  This is `source_id_entity_index`, not the
            // Castle's presentation/owner field.
            yaw = i16::try_from(heading_for_positions(predicted, target.position))
                .expect("mana-sphere defender heading fits");
            let distance = level2_wrapping_target_distance(predicted, target.position);
            let attraction_limit = if terrain.map_type == MapType::Cave {
                2_048
            } else {
                1_024
            };
            // `AddBallon_60AB0` jumps directly to LABEL_20 when the sphere's
            // raw player relation does not match the holder id.  That path
            // still runs the shared settle tail, but it does not perform the
            // target-distance snap or `MoveEntity` step.
            let relation_matches = target.player_entity_index == child_snapshot.id_entity_index;
            if relation_matches {
                if distance > attraction_limit {
                    // The source clears byte-0 bit 0x40 when the target is
                    // outside the Day/Cave attraction range, then continues
                    // through the normal movement/settle tail.
                    consumer.set_attraction(self, target.source_slot, false, source_slot);
                } else {
                    consumer.set_attraction(self, target.source_slot, true, source_slot);
                    if level2_castle_collector_target_overlap(predicted, &target) {
                        let child = &mut self.castle_children[child_index];
                        child.mana = child.mana.wrapping_add(target.mana);
                        child.player_entity_index = target.player_entity_index;
                        child.life = child.max_life;
                        child.target_sphere_slot = 0;
                        consumer.hide(self, target.source_slot);
                        if target.source_model == 39 {
                            self.events.push(Level2EconomyEvent::CastleDefenderCollectedSphere {
                                defender_slot: source_slot, sphere_id: u32::from(target.source_slot), mana: target.mana,
                            });
                        }
                    }
                }
                if distance <= u32::from(child_snapshot.speed.unsigned_abs()) {
                    predicted.x = target.position.x;
                    predicted.y = target.position.y;
                } else {
                    crate::move_entity(&mut predicted, yaw, 0, child_snapshot.speed);
                }
            }
        } else {
            // The physical target can be a freed or reused non-class3/10
            // row. Source still turns and moves toward its retained pose.
            yaw = i16::try_from(heading_for_positions(predicted, target.position))
                .expect("Castle defender heading fits");
            crate::move_entity(&mut predicted, yaw, 0, child_snapshot.speed);
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
        let child = &mut self.castle_children[child_index];
        child.position = predicted;
        child.yaw = yaw as u16;
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
        self.finish_castle_defender_source_damage(child_index, consumer);
        let child = self.castle_children[child_index];
        self.sync_castle_child_map(child);
        self.refresh_owner_mana_rows();
        true
    }

    fn finish_castle_defender_source_damage(&mut self, child_index: usize,
        consumer: &mut dyn Level2CastleCollectorConsumer) {
        let child = &mut self.castle_children[child_index];
        if child.life < 0 || child.pending_attacker_entity_index == 0 { return; }
        let attacker = child.pending_attacker_entity_index;
        let damage = child.pending_damage;
        child.life = child.life.wrapping_sub(damage);
        consumer.record_damage();
        let lethal = child.life < 0;
        if lethal {
            child.killer_entity_index = attacker;
        } else {
            child.pending_attacker_entity_index = 0;
        }
        self.events.push(Level2EconomyEvent::DamageApplied {
            target_entity_index: child.source_entity_index,
            attacker_entity_index: attacker,
            damage,
            lethal,
        });
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
        self.merge_mana_spheres_with_entities(target_slot, source_slot, None)
    }

    fn merge_mana_spheres_with_entities(
        &mut self,
        target_slot: u16,
        source_slot: u16,
        entities: Option<&mut [EntitySnapshot]>,
    ) -> bool {
        self.merge_mana_spheres_with_owner_resources(target_slot, source_slot, entities,
            &|economy, rows, slot| economy.current_owner_resources(rows, slot))
    }

    pub(crate) fn current_owner_resources(&self, entities: &[EntitySnapshot], slot: u16) -> (u8, i32) {
        let raw = entities.get(usize::from(slot));
        let Some(current) = self.live_arena_cursor_row(slot) else {
            return raw.map_or_else(|| (self.owner_class.get(&slot).copied().unwrap_or_default(),
                self.owner_max_mana.get(&slot).copied().unwrap_or_default()), |row| (row.class, row.max_mana));
        };
        let class = self.live_arena_class_action(slot, current.kind).map(|(class, _)| class)
            .or_else(|| raw.map(|row| row.class)).unwrap_or_default();
        let max_mana = match current.kind {
            Level2DynamicArenaKind::CastleRoot => self.castle_at(slot).map(|row| row.mana_capacity),
            Level2DynamicArenaKind::CastleDefender => self.castle_children.iter()
                .find(|row| row.active && row.source_entity_index == slot).map(|row| row.max_mana),
            Level2DynamicArenaKind::ManaCarrier => self.carriers.iter()
                .find(|row| row.active && row.source_entity_index == slot).map(|row| row.max_mana),
            _ => None,
        }.or_else(|| raw.map(|row| row.max_mana))
            .or_else(|| self.owner_max_mana.get(&slot).copied()).unwrap_or_default();
        (class, max_mana)
    }

    fn merge_mana_spheres_with_owner_resources(
        &mut self,
        target_slot: u16,
        source_slot: u16,
        mut entities: Option<&mut [EntitySnapshot]>,
        owner_resources: &impl Fn(&Level2EconomyState, &[EntitySnapshot], u16) -> (u8, i32),
    ) -> bool {
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
        // The source dereferences the current numeric occupant, not the class
        // that happened to occupy this slot at reset. A death marker is a
        // typed 10/40 even while its raw shadow has not yet been published;
        // after reclaim/reuse the current carrier or raw row takes precedence.
        let rows = entities.as_deref().unwrap_or(&[]);
        let (target_class, target_max_mana) = owner_resources(self, rows, target_relation);
        let (source_class, source_max_mana) = owner_resources(self, rows, source_relation);
        // `sub_36D50` arbitrates through the sole source
        // `playerEntityIndex_0x94` relation.  The Rust semantic owner may
        // intentionally differ (for example after a Nyphur relation write),
        // so select a side by the raw relation and copy both projections from
        // that winning sphere instead of treating either field as an alias.
        let source_wins = if target_relation == 0 {
            source_relation != 0
        } else if source_relation == 0 || target_relation == source_relation {
            false
        } else if target_class == 10 && source_class == 10
        {
            false
        } else if target_class == 10 && source_class != 10
        {
            true
        } else if source_class == 10 && target_class != 10
        {
            false
        } else if target_max_mana > source_max_mana
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
        if let Some(raw) = entities.as_deref_mut().and_then(|rows| rows.get_mut(usize::from(source_slot))) {
            // sub_57F20 clears the class but leaves this fixed record and its
            // next_0 readable until NewEvent overwrites the slot. A later
            // Castle scan tests model39 even when this record is class zero.
            let source = self.spheres[source_index];
            raw.retain_mana_sphere_source(&source);
        }
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
            source_roll: 0,
            source_fov: 0,
            // The production holder adapter binds particle 18 immediately
            // after allocation, before the new flyer can be scheduled.
            collision_shape: crate::combat::area::Shape {
                half_width: 0, half_depth: 0, vertical_offset: 0, half_height: 0,
            },
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

    pub(crate) fn set_castle_projectile_shape(
        &mut self,
        slot: u16,
        shape: crate::combat::area::Shape,
    ) -> bool {
        let Some(projectile) = self.castle_projectiles.iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)
        else { return false };
        projectile.collision_shape = shape;
        true
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
        let target = self.castle_projectiles.iter()
            .find(|row| row.active && row.source_entity_index == source_slot)
            .and_then(|row| {
                if row.source_word_96 == 0 { return None; }
                self.castle_entry(row.source_word_96).map(|castle| (castle.position,
                    crate::combat::area::Shape { vertical_offset: castle.shift_yaw,
                        half_width: castle.shift_pitch, half_depth: castle.shift_roll,
                        half_height: castle.shift_fov }))
                    .or_else(|| entities.get(usize::from(row.source_word_96)).map(|raw| (raw.position,
                        crate::combat::area::Shape { vertical_offset: raw.array_yaw(),
                            half_width: raw.array_pitch(), half_depth: raw.array_roll(),
                            half_height: raw.array_fov() })))
            });
        self.step_castle_projectile_slot_with_target(source_slot, entities, terrain, target).live
    }

    /// `target` is the current source record at word96, resolved by the
    /// arena owner before this visit. Nonzero fixed addresses remain valid
    /// even when their record is freed or occupied by a different entity.
    pub(crate) fn step_castle_projectile_slot_with_target(
        &mut self,
        source_slot: u16,
        entities: &[EntitySnapshot],
        terrain: &SessionTerrain,
        target: Option<(Position, crate::combat::area::Shape)>,
    ) -> CastleProjectileVisit {
        self.step_castle_projectile_slot_with_target_and_site_rows(
            source_slot, entities, terrain, target, None,
        )
    }

    pub(crate) fn step_castle_projectile_slot_with_target_and_site_rows(
        &mut self,
        source_slot: u16,
        entities: &[EntitySnapshot],
        terrain: &SessionTerrain,
        target: Option<(Position, crate::combat::area::Shape)>,
        site_rows: Option<&CastleSiteRows>,
    ) -> CastleProjectileVisit {
        let Some(projectile) = self
            .castle_projectiles
            .iter()
            .find(|projectile| projectile.active && projectile.source_entity_index == source_slot)
            .copied()
        else {
            return CastleProjectileVisit { live: false, clear_holder_owner: None };
        };
        if projectile.source_word_96 == 0
            && projectile.control_flags & LEVEL2_CASTLE_CONSTRUCTION_INITIALIZED_BIT == 0
        {
            if let Some(projectile) = self.castle_projectiles.iter_mut().find(|projectile| {
                projectile.active && projectile.source_entity_index == source_slot
            }) {
                projectile.control_flags |= LEVEL2_CASTLE_CONSTRUCTION_INITIALIZED_BIT;
            }
            if self.castle_projectile_site_blocked_with_rows(projectile.position, entities, terrain, site_rows) {
                self.disable_castle_projectile(source_slot, true);
                return CastleProjectileVisit { live: false, clear_holder_owner: Some(projectile.owner_entity_index) };
            }
            return CastleProjectileVisit { live: true, clear_holder_owner: None };
        }
        let target_position = target.map_or(projectile.axis_position, |target| target.0);
        let (landed, _gate_blocked, landing_position) = {
            let mut moved = projectile;
            let mut flight = crate::spells::castle::CastleFlight {
                position: moved.position, yaw: moved.yaw, pitch: moved.pitch,
                speed: moved.actual_speed, minimum_speed: moved.minimum_speed,
            };
            (moved.source_roll, moved.source_fov) = flight.advance(target_position);
            moved.position = flight.position;
            moved.yaw = flight.yaw;
            moved.pitch = flight.pitch;
            moved.actual_speed = flight.speed;
            let castle_hit = target.filter(|(position, shape)| {
                projectile.source_word_96 != 0
                    && level2_model76_sub106c0_matches(
                        moved.position,
                        moved.collision_shape.vertical_offset,
                        moved.collision_shape.half_width,
                        moved.collision_shape.half_depth,
                        moved.collision_shape.half_height,
                        *position,
                        shape.vertical_offset,
                        shape.half_width,
                        shape.half_depth,
                        shape.half_height,
                    )
            });
            if let Some((position, _)) = castle_hit {
                moved.position = position;
            }
            let mut terrain_hit = false;
            if castle_hit.is_none() {
                let ground = terrain.ground_height(moved.position);
                let ceiling = (terrain.map_type == MapType::Cave)
                    .then(|| terrain.ceiling_height(moved.position)).flatten();
                if let Some(height) = crate::spells::castle::terrain_contact(
                    moved.position, ground, ceiling, moved.collision_shape,
                ) {
                    moved.position.z = height;
                    terrain_hit = true;
                }
            }
            let hit = castle_hit.is_some() || terrain_hit;
            if !hit {
                moved.life = moved.life.wrapping_sub(1);
            }
            let collision_landed = hit || moved.life < 0;
            // `sub_66D00` re-runs `sub_11CB0` after movement.  A failed
            // later gate is reached only after the castle-hit, terrain, and
            // life branches have all failed.  It is then itself the source
            // landing predicate: rotate yaw by 0x400 and perform one
            // same-speed bounce before allocation.
            let gate_blocked = !collision_landed
                && projectile.source_word_96 == 0
                && self.castle_projectile_site_blocked_with_rows(moved.position, entities, terrain, site_rows);
            let landed = collision_landed || gate_blocked;
            if gate_blocked {
                let reverse_yaw = (moved.yaw.wrapping_add(0x400)) & 0x07ff;
                crate::move_entity(
                    &mut moved.position,
                    reverse_yaw,
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
            return CastleProjectileVisit { live: true, clear_holder_owner: None };
        }
        if projectile.source_byte_43 == 3 && projectile.source_byte_44 == 2 {
            if projectile.source_word_96 != 0 && self.castle_for_owner(projectile.owner_entity_index).is_some() {
                self.disable_castle_projectile(source_slot, false);
                return CastleProjectileVisit { live: false, clear_holder_owner: None };
            }
            let footprint = self.castle_stage_footprints.get(&1).cloned();
            let Some(footprint) = footprint else {
                return CastleProjectileVisit { live: true, clear_holder_owner: None };
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
                return CastleProjectileVisit { live: true, clear_holder_owner: (projectile.source_word_96 != 0).then_some(projectile.owner_entity_index) };
            }
        } else if self
            .allocate_castle_upgrade_product(landing_position, projectile.owner_entity_index)
            .is_none()
        {
            return CastleProjectileVisit { live: true, clear_holder_owner: (projectile.source_word_96 != 0).then_some(projectile.owner_entity_index) };
        }
        self.disable_castle_projectile(source_slot, false);
        CastleProjectileVisit { live: false, clear_holder_owner: None }
    }

    fn allocate_castle_upgrade_product(
        &mut self,
        position: Position,
        owner_entity_index: u16,
    ) -> Option<u16> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let product = Level2CastleChild {
            source_entity_index,
            entity_rng,
            id_entity_index: owner_entity_index,
            source_class: 10,
            // sub_502B0 leaves NewEvent's parent zero. 389F0 resolves the
            // owner's current Castle rather than storing a root relation.
            parent_entity_index: 0,
            owner_entity_index,
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

    /// Complete sub_502B0's particle-41 binding before the product's source
    /// visit. SetEntityShiftRot replaces its three horizontal/FOV extents but
    /// leaves the particle's vertical offset intact.
    pub(crate) fn bind_castle_upgrade_particle(
        &mut self,
        slot: u16,
        particle: &ParticleParameter,
    ) -> bool {
        let Some(product) = self.castle_children.iter_mut().find(|child| {
            child.active && child.source_entity_index == slot && child.model == 43 && child.action == 45
        }) else { return false };
        product.visual_index = 41;
        product.yaw = particle.rotation_speed / 2;
        true
    }

    /// Source `sub_11CB0`/`sub_11C80`'s first flyer-tick placement gate for
    /// the non-cave Day route. Production callers supply the current rows
    /// reached through the frozen source heads. BUILD payloads are not
    /// placement validity.
    pub(crate) fn castle_projectile_site_blocked(
        &self,
        position: Position,
        entities: &[EntitySnapshot],
        terrain: &SessionTerrain,
    ) -> bool {
        self.castle_projectile_site_blocked_with_rows(position, entities, terrain, None)
    }

    fn castle_projectile_site_blocked_with_rows(
        &self,
        position: Position,
        entities: &[EntitySnapshot],
        terrain: &SessionTerrain,
        site_rows: Option<&CastleSiteRows>,
    ) -> bool {
        let start_x = (position.x >> 8) as u8;
        let start_y = (position.y >> 8) as u8;
        let nearby = if let Some(rows) = site_rows {
            rows.class3.iter().any(|row| row.model == 2 && castle_site_row_nearby(row, position))
                || rows.model45.iter().any(|row| castle_site_row_nearby(row, position))
        } else {
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
            nearby_castle || nearby_model45
        };
        if nearby {
            return true;
        }
        for row in 0..8_u8 {
            for column in 0..8_u8 {
                let flags = terrain.angle_flags(crate::Tile {
                    x: start_x.wrapping_sub(8).wrapping_add(column),
                    y: start_y.wrapping_sub(8).wrapping_add(row),
                });
                if flags.cast_signed() < 0 || (terrain.map_type == MapType::Cave && flags & 8 != 0) {
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

    pub(super) fn castle_holder_source_visit(&self, slot: u16) -> Option<(u16, bool)> {
        let castle = self.castle_at(slot)?;
        let busy = match (castle.action, castle.build_state) {
            (5, 2) => false,
            (5, 3 | 5) => true,
            (4, _) if castle.timer > 1 => true,
            _ => return None,
        };
        Some((castle.source_id_entity_index, busy))
    }

    /// `sub_6D880(root)` in state 2 clears root word2c after the caller
    /// has resolved its actual Castle holder. At that source point word2e
    /// is still 2, so `SetSpell` only requeues word2c before clearing it.
    pub(super) fn clear_castle_release_cooldown(&mut self, slot: u16) -> bool {
        let Some(castle) = self.castle_at_mut(slot)
            .filter(|castle| castle.action == 5 && castle.build_state == 2)
        else {
            return false;
        };
        castle.auxiliary_cooldown = 0;
        true
    }

    pub(super) fn take_castle_upgrade_holder_clear_owners(&mut self) -> Vec<u16> {
        let pending = self.live_castles()
            .filter(|castle| castle.upgrade_holder_clear_pending)
            .map(|castle| (castle.source_entity_index, castle.source_id_entity_index))
            .collect::<Vec<_>>();
        let mut owners = Vec::with_capacity(pending.len());
        for (slot, owner) in pending {
            if let Some(castle) = self.castle_at_mut(slot) {
                castle.upgrade_holder_clear_pending = false;
                owners.push(owner);
            }
        }
        owners
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
        if owner_entity_index == self.local_player_entity_index && !source_constructor {
            self.player_castle_entity_index = source_entity_index;
            self.player_castle_stage = 1;
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
            castle.timer = castle_timer(&root);
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

    /// `sub_14E10` creates the same source seed as flyer landing, but binds
    /// its AI owner immediately, before the wizard's remaining callback.
    pub(crate) fn allocate_ai_castle_root_seed(
        &mut self,
        position: Position,
        owner_entity_index: u16,
        source_id: u16,
        terrain: &SessionTerrain,
    ) -> Option<Level2CastleState> {
        let footprint = self.castle_stage_footprints.get(&1)?;
        let (root_position, anchor) = level2_castle_root_positions(terrain, footprint, position);
        let slot = self.allocate_castle_root_seed(root_position, anchor, owner_entity_index)?;
        self.castle_at_mut(slot)?.source_id_entity_index = source_id;
        self.link_castle_owner(owner_entity_index, slot);
        self.castle_at(slot).copied()
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

    #[cfg(test)]
    pub(super) fn select_castle_defender_sphere(
        &self,
        holder_position: Position,
        holder_id_entity_index: u16,
        exclude_one: u16,
        exclude_two: u16,
    ) -> Option<u16> {
        self.select_castle_defender_sphere_with_consumer(holder_position, holder_id_entity_index,
            exclude_one, exclude_two, &|_: &mut Level2EconomyState, _: Level2CastleState| {})
    }

    fn select_castle_defender_sphere_with_consumer(
        &self,
        holder_position: Position,
        holder_id_entity_index: u16,
        exclude_one: u16,
        exclude_two: u16,
        consumer: &dyn super::player_spell_state::CastleSourceConsumer,
    ) -> Option<u16> {
        let mut best = None;
        let mut best_distance = u32::MAX;
        for sphere in consumer.mana_rows(self) {
            if sphere.player_entity_index != holder_id_entity_index
                || sphere.control_flags & LEVEL2_MANA_SPHERE_TARGET_SUPPRESSION_BIT != 0
                || sphere.source_slot == exclude_one || sphere.source_slot == exclude_two
            { continue; }
            let distance = level2_wrapping_source_distance_3d(holder_position, sphere.position);
            if distance < best_distance {
                best_distance = distance;
                best = Some(sphere.source_slot);
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
    #[cfg(test)]
    pub(super) fn maintain_castle_defenders(&mut self, castle_slot: u16, terrain: &SessionTerrain) {
        self.maintain_castle_defenders_with_consumer(castle_slot, terrain,
            &|_: &mut Level2EconomyState, _: Level2CastleState| {});
    }

    fn maintain_castle_defenders_with_consumer(&mut self, castle_slot: u16, terrain: &SessionTerrain,
        consumer: &dyn super::player_spell_state::CastleSourceConsumer) {
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
                    if let Some(sphere_slot) = self.select_castle_defender_sphere_with_consumer(
                        child.position,
                        child.id_entity_index,
                        exclude_one,
                        exclude_two,
                        consumer,
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

    /// Pure `sub_11A10` probe. Membership is the class-3 list frozen at the
    /// source prepass; pointed-to geometry is resolved at this visit. The
    /// four map strips below deliberately preserve the source asymmetry.
    pub(crate) fn castle_upgrade_site_clear(
        &self,
        castle_slot: u16,
        terrain: &SessionTerrain,
        class3_slots: &[u16],
        entities: &[EntitySnapshot],
    ) -> bool {
        let Some(castle) = self.castle_at(castle_slot) else {
            return false;
        };
        self.castle_upgrade_site_clear_for(*castle, terrain, class3_slots, entities)
    }

    pub(crate) fn castle_upgrade_site_checks(
        &self,
        castle_slot: u16,
        terrain: &SessionTerrain,
        class3_slots: &[u16],
        entities: &[EntitySnapshot],
    ) -> [bool; 2] {
        let Some(mut castle) = self.castle_at(castle_slot).copied() else {
            return [false; 2];
        };
        let first = self.castle_upgrade_site_clear_for(castle, terrain, class3_slots, entities);
        (castle.shift_pitch, castle.shift_roll) = level2_castle_shift_axes(castle.level);
        let following = self.castle_upgrade_site_clear_for(castle, terrain, class3_slots, entities);
        [first, following]
    }

    fn castle_upgrade_site_clear_for(
        &self,
        castle: Level2CastleState,
        terrain: &SessionTerrain,
        class3_slots: &[u16],
        entities: &[EntitySnapshot],
    ) -> bool {
        let (pitch, roll) = level2_castle_shift_axes(castle.level.wrapping_add(1));
        for &slot in class3_slots {
            if slot == castle.source_entity_index {
                continue;
            }
            let geometry = if let Some(other) = self.castle_entry(slot) {
                (other.source_model == 2).then_some((
                    other.position, other.shift_yaw, other.shift_pitch,
                    other.shift_roll, other.shift_fov,
                ))
            } else {
                entities.get(usize::from(slot)).filter(|row| row.model == 2).map(|row| (
                    row.position, row.array_yaw(), row.array_pitch(), row.array_roll(), row.array_fov(),
                ))
            };
            if let Some((position, yaw, other_pitch, other_roll, fov)) = geometry
                && level2_model76_sub106c0_matches(
                    castle.position, 0, pitch, roll, 256,
                    position, yaw, other_pitch, other_roll, fov,
                )
            {
                return false;
            }
        }
        let old_x = castle.shift_pitch >> 8;
        let old_y = castle.shift_roll >> 8;
        let next_x = pitch >> 8;
        let next_y = roll >> 8;
        let dx = next_x.wrapping_sub(old_x);
        let dy = next_y.wrapping_sub(old_y);
        let x = (castle.position.x.wrapping_add(128) >> 8) as u8;
        let y = (castle.position.y.wrapping_add(128) >> 8) as u8;
        let start_x = x.wrapping_sub(next_x as u8);
        let start_y = y.wrapping_sub(next_y as u8);
        let strip_clear = |x: u8, y: u8, reset_x: u8, width: i16, rows: i16| {
            // Source decrement-to-zero shorts may start negative (stage7
            // probes BUILD entry8). The read-only map repeats every256 cells
            // on each axis, so repeated cycles cannot change this predicate.
            let columns = u16::from_ne_bytes(width.to_ne_bytes()).min(256);
            let rows = u16::from_ne_bytes(rows.to_ne_bytes()).min(if x == reset_x { 256 } else { 257 });
            let mut row_y = y;
            for row in 0..rows {
                let mut column_x = if row == 0 { x } else { reset_x };
                for _ in 0..columns {
                    let angle = terrain.planes.angles[terrain_index(column_x, row_y)] as i8;
                    if angle < 0 || terrain.map_type == MapType::Cave && angle & 8 != 0 {
                        return false;
                    }
                    column_x = column_x.wrapping_add(1);
                }
                row_y = row_y.wrapping_add(1);
            }
            true
        };
        strip_clear(start_x, start_y, start_x, next_x.wrapping_mul(2), dy)
            && strip_clear(start_x, start_y.wrapping_add((2 * next_y - dy) as u8), start_x, next_x.wrapping_mul(2), dy)
            && strip_clear(start_x, start_y.wrapping_add(dy as u8), start_x, dx, dy)
            // v11 starts at the right x for its first row only. The source
            // resets subsequent rows to v20's LEFT x, just like v8.
            && strip_clear(start_x.wrapping_add((next_x - dx) as u8), start_y.wrapping_add(dy as u8), start_x, dx, dy)
    }

    /// Every return from `sub_11A10` runs SetShiftByCastle(current stage),
    /// including its yaw/fov reset rather than the action-4 vertical envelope.
    pub(crate) fn restore_castle_site_shifts(&mut self, castle_slot: u16) {
        if let Some(castle) = self.castle_entry_mut(castle_slot) {
            (castle.shift_pitch, castle.shift_roll) = level2_castle_shift_axes(castle.level);
            castle.shift_yaw = 0;
            castle.shift_fov = 256;
        }
    }

    /// Advance the bounded Castle root action. This follows the source state
    /// table in `BeginOfCastleCreation_5FA70`: states 0/3 allocate a class-10
    /// model-42 child, state 4 scans for that child, and state 5 allocates the
    /// model-41 child. The model-42 allocation is atomic; the bounded
    /// model-79 stage-controller loop may commit a partial chain when the
    /// source arena exhausts.
    #[cfg(test)]
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
    #[cfg(test)]
    pub(crate) fn step_castle_action_with_final_loss(
        &mut self,
        castle_slot: u16,
        terrain: &mut SessionTerrain,
        reset_free_slots: &[u16],
        on_final_loss: &mut dyn FnMut(&mut Level2EconomyState, Level2CastleState),
    ) -> bool {
        let class3_slots = self.live_castles().map(|castle| castle.source_entity_index).collect::<Vec<_>>();
        let site_clear = self.castle_at(castle_slot).is_none_or(|castle| {
            castle.action != 5 || castle.build_state != 0 || castle.level == 0
                || self.castle_upgrade_site_checks(castle_slot, terrain, &class3_slots, &[])[1]
        });
        let mut consumer = |economy: &mut Level2EconomyState, castle: Level2CastleState| on_final_loss(economy, castle);
        self.step_castle_action_with_site(castle_slot, terrain, reset_free_slots, site_clear, &mut consumer)
    }

    pub(crate) fn step_castle_action_with_site(
        &mut self,
        castle_slot: u16,
        terrain: &mut SessionTerrain,
        reset_free_slots: &[u16],
        site_clear: bool,
        consumer: &mut dyn super::player_spell_state::CastleSourceConsumer,
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
                if snapshot.build_state == 0 {
                    // 5FA70: level-zero creation bypasses 11A10; upgrades
                    // recheck after the paid flyer has reached this root.
                    self.restore_castle_site_shifts(castle_slot);
                    if snapshot.level != 0 && !site_clear {
                        let castle = self.castle_entry_mut(castle_slot).expect("Castle remains installed");
                        castle.build_state = 2;
                        castle.source_word_2e = 2;
                        castle.control_flags &= !0x40;
                        self.advance_castle_phase(castle_slot);
                        return true;
                    }
                }
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
                castle.source_word_2e = 0;
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
                self.castle_entry_mut(castle_slot)
                    .expect("Castle remains installed")
                    .position
                    .z = terrain.ground_height(snapshot.position);
                // BeginOfCastleCreation_5FA70 scans the class-10/model-42
                // list only every 32 source visits. The ordinary epilogue
                // still advances phase on the intervening visits.
                if snapshot.phase & 0x1f != 0 || self
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
            4 if snapshot.timer > 0 => {
                let castle = self.castle_entry_mut(castle_slot).expect("Castle remains installed");
                if castle.timer == 1 {
                    // EndOfCastleProjectile tests bit 0x10 of control byte 2;
                    // while set it holds the timer at one without grounding.
                    if castle.control_flags & 0x0010_0000 == 0 {
                        castle.timer = 0;
                        castle.action = 5;
                        castle.build_state = 3;
                    }
                } else {
                    castle.timer = castle.timer.wrapping_sub(1);
                    castle.position.z = terrain.ground_height(castle.position);
                }
                castle.source_word_30 = castle.timer;
                self.advance_castle_phase(castle_slot);
                true
            }
            4 => {
                let source_phase = snapshot.phase;
                if snapshot.life >= 0 && snapshot.pending_attacker_entity_index != 0 {
                    let _ = self.resolve_damage(castle_slot);
                }
                let castle = self.castle_entry_mut(castle_slot).expect("Castle remains installed");
                if castle.life < 0 {
                    castle.action = 6;
                } else {
                    // sub_609E0 consumes word80 independently of word7c,
                    // after damage, and leaves word7c intact.
                    if castle.source_word_80 == castle.source_id_entity_index {
                        castle.source_word_80 = 0;
                        if castle.level < 7 {
                            castle.control_flags |= 0x40;
                            castle.upgrade_holder_clear_pending = true;
                        }
                    }
                    if castle.control_flags & 0x40 != 0 {
                        castle.action = 5;
                        castle.build_state = 0;
                        castle.source_word_2e = 0;
                    }
                }
                // The action transition does not skip the current callback's
                // grounding or even-phase economy maintenance.
                castle.position.z = terrain.ground_height(castle.position);
                if source_phase & 1 != 0 {
                    self.advance_castle_phase(castle_slot);
                    return true;
                }
                self.release_castle_excess_mana(castle_slot, terrain, reset_free_slots);
                // EndOfCastleProjectile refreshes the current stage's BUILD
                // footprint before defender maintenance and sphere collection.
                let (shift_pitch, shift_roll) = level2_castle_shift_axes(snapshot.level);
                if let Some(castle) = self.castle_entry_mut(castle_slot) {
                    castle.shift_pitch = shift_pitch;
                    castle.shift_roll = shift_roll;
                    castle.shift_yaw = -8_192;
                    castle.shift_fov = 0x4000;
                }
                self.maintain_castle_defenders_with_consumer(castle_slot, terrain, consumer);
                let Some(castle) = self.castle_entry(castle_slot).copied() else {
                    self.advance_castle_phase(castle_slot);
                    return false;
                };
                if castle.mana >= castle.mana_capacity {
                    self.advance_castle_phase(castle_slot);
                    return true;
                }
                // The class-10 mana list is rebuilt in numeric slot order at
                // the source prepass; a same-pass birth is not linked into it.
                let Some(sphere) = consumer.mana_rows(self).into_iter()
                    .find(|sphere| sphere.player_entity_index == castle.source_id_entity_index
                        && level2_castle_sphere_overlap(&castle, sphere))
                else {
                    self.advance_castle_phase(castle_slot);
                    return true;
                };
                let castle = self.castle_entry_mut(castle_slot).expect("Castle remains installed");
                castle.mana = castle.mana.wrapping_add(sphere.mana);
                let carrier_id = u32::from(castle.source_entity_index);
                consumer.hide_mana_source(self, sphere.source_slot);
                self.events.push(Level2EconomyEvent::ManaSphereCollected {
                    sphere_id: u32::from(sphere.source_slot),
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
                    consumer.final_loss(self, final_castle);
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
                self.maintain_castle_defenders_with_consumer(castle_slot, terrain, consumer);
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
            // sub_609E0 clears nonlethal damage independently of the
            // separate word80 upgrade mailbox.
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
