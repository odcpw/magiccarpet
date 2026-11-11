//! Source1D5D0(184), over the current arena record used by ordinary model23.

use super::{Effects, RULES, SOURCE_MODEL};
use crate::campaign::source_arena::SourceArena;
use crate::native_world::creatures::archer::{ArcherStageDescriptor, ArcherStageRuntime};
use crate::native_world::{
    EntitySnapshot, ParticleParameter, RuntimeSnapshot, SessionTerrain, source_target,
};
use crate::{FlightWorld, Position, heading_for_positions};

fn word(row: &EntitySnapshot, at: usize) -> u16 {
    u16::from_le_bytes([row.record[at], row.record[at + 1]])
}
fn put(row: &mut EntitySnapshot, at: usize, value: u16) {
    row.record[at..at + 2].copy_from_slice(&value.to_le_bytes());
}
fn remaining(row: &EntitySnapshot) -> i16 {
    word(row, 50) as i16
}
fn set_remaining(row: &mut EntitySnapshot, value: i16) {
    put(row, 50, value as u16);
}
fn decrement_remaining(row: &mut EntitySnapshot, amount: i16) {
    let value = remaining(row).wrapping_sub(amount);
    set_remaining(row, value);
}

struct Stage<'a, 'b, 'c> {
    arena: &'a mut SourceArena,
    runtime: &'a mut RuntimeSnapshot,
    terrain: &'a mut SessionTerrain,
    particles: &'a [ParticleParameter],
    base: source_target::Context<'b>,
    players: &'a mut source_target::Creature23PlayerFeedback<'c>,
    class3: &'a [u16],
    families: &'a [Vec<u16>; 29],
}

impl Stage<'_, '_, '_> {
    fn packet(&self, slot: u16) -> Result<EntitySnapshot, String> {
        if slot == 0 {
            return self
                .arena
                .entities
                .first()
                .cloned()
                .ok_or("model23 stage lacks sentinel".into());
        }
        source_target::status_parent(self.arena, self.players.context(self.base), slot)?
            .ok_or_else(|| format!("model23 stage address {slot} is outside the arena"))
    }

    fn publish(&mut self, row: &EntitySnapshot) -> Result<(), String> {
        crate::world::effects::commit_entity(self.arena, self.terrain, row.clone())
    }

    fn movement(&mut self, row: &mut EntitySnapshot) -> Result<(), String> {
        super::move_source_actor(
            row,
            self.terrain,
            Some(&mut self.arena.economy.source_creature_population),
        );
        self.publish(row)
    }

    fn descriptor(&self, row: &EntitySnapshot) -> Result<ArcherStageDescriptor, String> {
        self.runtime
            .descriptor(row.record[76])
            .ok_or_else(|| format!("model23 stage descriptor {} is absent", row.record[76]))
    }

    fn consume(&mut self, row: &mut EntitySnapshot) -> Result<u8, String> {
        let mailbox = word(row, 102);
        let (mut attacker, mut hit) = (0, 0);
        if mailbox != 0 {
            let damage =
                i32::from_le_bytes(row.record[98..102].try_into().expect("source damage dword"));
            row.set_life(row.life.wrapping_sub(damage));
            row.clear_source_upkeep_owner();
            attacker = mailbox;
            hit = 1;
        }
        // DDA0/D8C0/E1C0 publish the mailbox and word26 before traversing
        // children or reaching any later fallible descriptor/target read.
        row.set_damage_result_words(row.word_24(), attacker);
        self.publish(row)?;
        let (mut child, mut visits) = (row.word_34(), 0);
        while child != 0 {
            if visits >= self.arena.entities.len() {
                return Err("model23 stage damage chain cycles".into());
            }
            let current = self.packet(child)?;
            if current.life < row.life {
                row.set_life(current.life);
                attacker = current.word_26();
                hit = 1;
                break;
            }
            child = current.word_34();
            visits += 1;
        }
        row.set_damage_result_words(
            if row.life < 0 {
                attacker
            } else {
                row.word_24()
            },
            attacker,
        );
        self.publish(row)?;
        Ok(if row.life < 0 { 2 } else { hit })
    }

    fn attack_action(&self, row: &mut EntitySnapshot) -> Result<(), String> {
        if RULES.behavior_flags & 8 != 0 {
            return Err("model23 configured flee descriptor selects unsupported action190".into());
        }
        row.set_source_action(186);
        Ok(())
    }

    fn hit(&self, row: &mut EntitySnapshot) -> Result<(), String> {
        let attacker = self.packet(row.word_26())?;
        if (attacker.class, attacker.model) != (row.class, row.model) {
            row.set_source_word_96(row.word_26());
            row.record[77] = 10;
            self.attack_action(row)?;
        }
        let mut position = row.position;
        crate::ground_creature_motion::relax_to_terrain(
            &mut position,
            self.terrain,
            RULES.ground_clearance,
            RULES.descent,
        );
        row.set_position(position);
        Ok(())
    }

    fn jitter(row: &mut EntitySnapshot) {
        let first = super::level2_next_rng16(row.entity_rng);
        let second = super::level2_next_rng16(first);
        row.set_entity_rng(second);
        let sign = 2 * (i32::from(first % 157) / 79) - 1;
        row.set_source_roll(
            (i32::from(row.source_roll()) + sign * (i32::from(second & 255) + 85)) as i16 & 2047,
        );
    }

    fn follow(
        &self,
        row: &mut EntitySnapshot,
        point: Position,
        signed: bool,
        target_width: Option<i16>,
    ) -> Result<(), String> {
        if row.phase & 7 != 0 {
            return Ok(());
        }
        if row.control_flags & 0x40000 == 0 {
            row.set_source_roll(heading_for_positions(row.position, point) as i16);
            if row.phase & 63 == 0 {
                Self::jitter(row);
            }
        }
        for &slot in &self.families[usize::from(SOURCE_MODEL)] {
            let peer = self.packet(slot)?;
            let delta = |a: u16, b: u16| {
                if signed {
                    (i32::from(a as i16) - i32::from(b as i16)).abs()
                } else {
                    (i32::from(a) - i32::from(b)).abs()
                }
            };
            if peer.source_id != row.source_id
                && delta(row.position.x, peer.position.x) < i32::from(row.array_pitch())
                && delta(row.position.y, peer.position.y) < i32::from(row.array_pitch())
            {
                row.set_source_roll(heading_for_positions(peer.position, row.position) as i16);
                if target_width.is_none() {
                    return Ok(());
                }
                break;
            }
        }
        // D8C0 runs target separation after its peer loop, unlike DDA0/E700.
        if let Some(width) = target_width {
            let spacing = i32::from(row.array_pitch()) + i32::from(width);
            if (i32::from(row.position.x) - i32::from(point.x)).abs() < spacing
                && (i32::from(row.position.y) - i32::from(point.y)).abs() < spacing
            {
                row.set_source_roll(heading_for_positions(point, row.position) as i16);
            }
        }
        Ok(())
    }

    fn idle(row: &mut EntitySnapshot) {
        if row.phase & 15 == 0 && row.control_flags & 0x40000 == 0 {
            row.set_entity_rng(super::level2_next_rng16(row.entity_rng));
            row.set_source_roll(
                row.source_roll()
                    .wrapping_add((row.entity_rng % 113 + 142) as i16)
                    & 2047,
            );
        }
    }

    fn selected(
        &self,
        row: &mut EntitySnapshot,
        descriptor: ArcherStageDescriptor,
    ) -> Result<u16, String> {
        if descriptor.flags & 2 == 0 {
            return Ok(descriptor.actor);
        }
        let mut selected = word(row, 78);
        if selected == 0 && descriptor.flags & 1 != 0 {
            for &slot in &self.families[usize::from(SOURCE_MODEL)] {
                let peer = self.packet(slot)?;
                if peer.record[76] == row.record[76] && word(&peer, 78) != 0 {
                    selected = word(&peer, 78);
                    break;
                }
            }
        }
        if selected == 0 {
            let family = self
                .families
                .get(usize::from(descriptor.model))
                .ok_or_else(|| format!("model23 stage model {} exceeds table", descriptor.model))?;
            let mut nearest = u32::MAX;
            for &slot in family {
                let current = self.packet(slot)?;
                let distance = super::xy_distance_squared(row.position, current.position);
                if distance < nearest {
                    nearest = distance;
                    selected = slot;
                }
            }
        }
        put(row, 78, selected);
        Ok(selected)
    }

    fn ordinary(
        &mut self,
        row: &mut EntitySnapshot,
        point_mode: bool,
        actor_target: bool,
    ) -> Result<(), String> {
        match self.consume(row)? {
            2 => row.set_source_action(188),
            1 => self.hit(row)?,
            _ if point_mode || actor_target || RULES.behavior_flags & 2 == 0 => {
                self.movement(row)?;
                if point_mode {
                    if row.phase & 7 == 0 {
                        let point = self.descriptor(row)?.point;
                        self.follow(row, point, true, None)?;
                    }
                } else if actor_target && row.phase & 7 == 0 {
                    let descriptor = self.descriptor(row)?;
                    let selected = self.selected(row, descriptor)?;
                    if selected != 0 {
                        let current = self.packet(selected)?;
                        self.follow(row, current.position, false, Some(current.array_pitch()))?;
                    } else {
                        Self::idle(row);
                    }
                } else if !actor_target {
                    Self::idle(row);
                }
            }
            _ => {}
        }
        self.publish(row)?;
        Ok(())
    }

    fn feedback(&mut self, parent: u16, family: u8) -> Result<(), String> {
        if !self.players.shared.mission_rewards.allows_experience() {
            return Ok(());
        }
        let current = self.packet(parent)?;
        if parent != 0 && (current.class, current.model) == (3, 0) {
            let crate::native_world::EntitySpecialRelation::PlayerState(index) =
                current.special_relation
            else {
                return Err(format!(
                    "model23 source6D8B0 parent {parent} lacks a current player spell A4"
                ));
            };
            let owner = self
                .players
                .shared
                .players
                .get(usize::from(index))
                .ok_or_else(|| {
                    format!("model23 source6D8B0 parent {parent} lacks PlayerState {index}")
                })?
                .entity_index;
            self.players.shared.award_spell_experience(owner, family, 1);
            crate::native_world::simulation::publish_family_spell_progression(
                self.players.shared,
                self.players.core_players,
                self.players.fire.as_deref_mut(),
                owner,
                family,
            );
        }
        Ok(())
    }

    fn summon_follow(&mut self, row: &mut EntitySnapshot) -> Result<(), String> {
        let hit = self.consume(row)?;
        if hit == 2 {
            set_remaining(row, 1);
            return Ok(());
        }
        self.movement(row)?;
        if hit == 1 {
            let attacker = self.packet(row.word_26())?;
            if row.word_26() != word(row, 44)
                && (attacker.class, attacker.model) != (row.class, row.model)
            {
                row.set_source_word_96(row.word_26());
                self.attack_action(row)?;
                self.feedback(word(row, 44), 19)?;
            }
        } else if row.phase & 7 == 0 && row.source_word_96() != 0 {
            let point = self.packet(row.source_word_96())?.position;
            self.follow(row, point, false, None)?;
        }
        Ok(())
    }

    fn blink(&mut self, row: &mut EntitySnapshot, clear: bool) -> Result<(), String> {
        let selection = if clear {
            0
        } else if i32::from(remaining(row)) <= i32::from(word(row, 52) as i16) - 16
            && remaining(row) >= 16
        {
            if row.phase & 0x1c == 16 {
                row.phase & 3
            } else {
                0
            }
        } else if row.phase & 1 == 0 {
            2
        } else {
            0
        };
        let bits = match selection {
            1 | 3 => 0x02000000,
            2 => 0x04000000,
            _ => 0,
        };
        row.set_control_flags((row.control_flags & !0x06000000) | bits);
        self.publish(row)?;
        let (mut next, mut visits) = (row.word_34(), 0);
        while next != 0 {
            if visits >= self.arena.entities.len() {
                return Err("model23 Alliance blink chain cycles".into());
            }
            let before = self.packet(next)?;
            next = before.word_34();
            let mut after = before.clone();
            after.set_control_flags((before.control_flags & !0x06000000) | bits);
            self.players.commit_flags(self.arena, &before, &after)?;
            visits += 1;
        }
        Ok(())
    }

    fn expire(&mut self, row: &EntitySnapshot) -> Result<(), String> {
        self.publish(row)?;
        let (slot, generation) = (row.index, self.arena.slot_generation(row.index));
        if row.record[77] == 13 {
            super::super::goat::runtime::spawn_expiry_visual(
                self.arena,
                self.terrain,
                row.position,
                self.particles,
            )?;
            if self.arena.slot_generation(slot) != generation {
                let position = Position {
                    z: self.terrain.ground_height(Position::default()),
                    ..Position::default()
                };
                crate::world::effects::sync_position(
                    self.arena,
                    self.terrain,
                    slot,
                    position,
                    true,
                    true,
                )?;
            }
        }
        let position = self.packet(slot)?.position;
        crate::world::effects::allocate_from_source_position(
            self.arena,
            self.terrain,
            sim_definition::adapter::mc2_effect_definition(
                crate::level_definition::EffectKind::Impact,
            ),
            slot,
            position,
            self.particles,
        )?;
        let before = self.packet(slot)?;
        let mut after = before.clone();
        after.disable_drawing();
        self.players.commit_flags(self.arena, &before, &after)
    }

    fn summoned(&mut self, row: &mut EntitySnapshot) -> Result<bool, String> {
        let parent_slot = word(row, 44);
        let parent = self.packet(parent_slot)?;
        if parent_slot == 0 || parent.life < 0 || parent.control_flags & 0x400 != 0 {
            set_remaining(row, 0);
        } else if row.record[77] == 13 {
            decrement_remaining(row, 1);
        }
        if remaining(row) <= 0 {
            self.expire(row)?;
            return Ok(false);
        }
        let mut target = row.source_word_96();
        if target != 0 {
            let current = self.packet(target)?;
            if current.life <= 0 || current.control_flags & 0x400 != 0 {
                target = 0;
                row.set_source_word_96(0);
            }
        }
        if target == 0 && row.phase & 7 == 0 {
            let mut nearest = u32::MAX;
            for &slot in self.class3 {
                let current = self.packet(slot)?;
                if current.source_id != parent.source_id && matches!(current.model, 0 | 1) {
                    let distance = super::xy_distance_squared(parent.position, current.position);
                    if distance < nearest {
                        nearest = distance;
                        target = slot;
                    }
                }
            }
            row.set_source_word_96(target);
        }
        if target == 0 {
            row.set_source_word_96(parent_slot);
            self.summon_follow(row)?;
            row.set_source_word_96(0);
            decrement_remaining(row, 4);
        } else {
            self.summon_follow(row)?;
            if row.phase & 7 == 0
                && super::distance(row.position, self.packet(target)?.position) < RULES.target_range
            {
                self.attack_action(row)?;
            }
        }
        Ok(true)
    }

    fn alliance(&mut self, row: &mut EntitySnapshot) -> Result<(), String> {
        decrement_remaining(row, 1);
        let parent_slot = word(row, 44);
        let mut ended = remaining(row) <= 0;
        if !ended {
            self.blink(row, false)?;
            let parent = self.packet(parent_slot)?;
            ended = parent_slot == 0 || parent.life <= 0;
            if !ended {
                let previous = row.source_word_96();
                row.set_source_word_96(parent_slot);
                self.publish(row)?;
                self.summon_follow(row)?;
                self.publish(row)?;
                let parent = self.packet(parent_slot)?;
                let inherited = if parent.source_word_96() != 0 {
                    parent.source_word_96()
                } else {
                    parent.word_26()
                };
                let mut selected = if inherited != 0 { inherited } else { previous };
                row.set_source_word_96(selected);
                if selected != 0 {
                    let current = self.packet(selected)?;
                    if (word(&current, 44) == parent_slot && current.record[77] == 14)
                        || current.life <= 0
                        || current.control_flags & 0x400 != 0
                    {
                        selected = 0;
                        row.set_source_word_96(0);
                    }
                    if selected != 0
                        && super::distance(row.position, current.position) < RULES.target_range
                    {
                        self.attack_action(row)?;
                        self.feedback(parent_slot, 24)?;
                    }
                }
            }
        }
        if ended {
            row.record[77] = 10;
            row.set_source_word_96(0);
            put(row, 44, 0);
            row.record[76] = word(row, 78) as u8;
            self.blink(row, true)?;
        }
        Ok(())
    }

    fn run(&mut self, row: &mut EntitySnapshot) -> Result<bool, String> {
        match row.record[77] {
            1..=9 => {
                let mode = row.record[77];
                // DBF0 reads the guard point before its damage helper.
                // DDA0/D8C0 instead read their descriptor only after damage,
                // movement and phase admission, at the sites inside ordinary.
                let point_mode = if mode == 2 {
                    let point = self.descriptor(row)?.point;
                    (i32::from(point.x as i16) - i32::from(row.position.x as i16)).abs() > 3072
                        || (i32::from(point.y as i16) - i32::from(row.position.y as i16)).abs()
                            > 3072
                } else {
                    mode == 1
                };
                self.ordinary(row, point_mode, matches!(mode, 3..=5))?;
                if matches!(mode, 3 | 4) && row.phase & 7 == 0 {
                    let d = self.descriptor(row)?;
                    let selected = if d.flags & 2 == 0 {
                        d.actor
                    } else {
                        word(row, 78)
                    };
                    if selected != 0 {
                        let current = self.packet(selected)?;
                        let selected = if mode == 4 {
                            current.source_word_96()
                        } else {
                            selected
                        };
                        if selected != 0
                            && (mode != 4 || selected != 0xae02)
                            && super::distance(row.position, self.packet(selected)?.position)
                                <= RULES.target_range
                        {
                            row.set_source_word_96(selected);
                            row.record[77] = 10;
                            self.attack_action(row)?;
                        }
                    }
                }
                if mode == 2 && row.action == 191 && row.record[77] == 2 && row.record[61] != 0 {
                    if RULES.scan_period == 0 {
                        return Err("model23 guard scan divisor is zero".into());
                    }
                    if row.phase % RULES.scan_period == 0 {
                        let (mut nearest, mut selected) = (u32::MAX, 0);
                        for &slot in self.class3 {
                            let current = self.packet(slot)?;
                            let x = i32::from(current.position.x) - i32::from(row.position.x);
                            let y =
                                i32::from(current.position.y.wrapping_sub(row.position.y) as i16);
                            let distance = x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y)) as u32;
                            let heading = heading_for_positions(row.position, current.position);
                            if distance <= RULES.target_range.wrapping_mul(RULES.target_range)
                                && distance < nearest
                                && current.control_flags & 0x20 == 0
                                && crate::angle_distance(i32::from(row.yaw), i32::from(heading))
                                    < RULES.target_half_angle
                            {
                                nearest = distance;
                                selected = slot;
                            }
                        }
                        if selected != 0 {
                            row.set_source_word_96(selected);
                            row.record[77] = 10;
                            self.attack_action(row)?;
                        }
                    }
                }
            }
            10 => self.attack_action(row)?,
            12 => {
                let parent_slot = word(row, 44);
                let parent = self.packet(parent_slot)?;
                if parent_slot == 0 || parent.life < 0 {
                    row.disable_drawing();
                    self.publish(row)?;
                }
                let position = Position {
                    z: parent.position.z.max(0),
                    ..parent.position
                };
                self.terrain.set_source_predicted_position(position);
                row.set_position(position);
                self.publish(row)?;
                // 1E4D0 reads the fixed parent's current pose after CopyEntityPosition.
                // The status packet only overlays some roll carriers; read the
                // canonical typed pose instead of its retained raw scalar.
                let (yaw, roll) = if parent_slot == 0 {
                    let sentinel = self.packet(0)?;
                    (sentinel.yaw, sentinel.source_roll())
                } else {
                    let current = source_target::read(
                        self.arena,
                        self.players.context(self.base),
                        parent_slot,
                    )?
                    .ok_or_else(|| format!("model23 follow parent {parent_slot} disappeared"))?;
                    (
                        current.yaw.ok_or_else(|| {
                            format!("model23 follow parent {parent_slot} has no current yaw")
                        })?,
                        current.roll.ok_or_else(|| {
                            format!("model23 follow parent {parent_slot} has no current roll")
                        })?,
                    )
                };
                row.set_yaw(yaw);
                row.set_source_roll(roll);
            }
            13 | 16 => return self.summoned(row),
            14 => self.alliance(row)?,
            17 => {
                self.movement(row)?;
                if row.life < 0 {
                    row.disable_drawing();
                } else {
                    let target_slot = row.source_word_96();
                    let target = self.packet(target_slot)?;
                    if target_slot == 0 || target.life < 0 || target.control_flags & 0x400 != 0 {
                        row.record[77] = 16;
                    } else {
                        row.set_source_roll(
                            heading_for_positions(row.position, target.position) as i16
                        );
                        row.set_actual_speed(row.actual_speed.wrapping_sub(8));
                        if row.actual_speed <= 16 {
                            row.record[77] = 16;
                        }
                    }
                }
            }
            // Source1D5D0 returns literally for 0,11,15 and other modes.
            _ => {}
        }
        Ok(true)
    }
}

pub(super) fn scheduled_step(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    source_context: source_target::Context<'_>,
    players: &mut source_target::Creature23PlayerFeedback<'_>,
    slot: u16,
    class3: &[u16],
    families: &[Vec<u16>; 29],
) -> Result<Effects, String> {
    let mut row = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && (row.class, row.model, row.action) == (5, SOURCE_MODEL, 191))
        .cloned()
        .ok_or_else(|| format!("model23 StageVar callback lost slot{slot}"))?;
    let mut stage = Stage {
        arena,
        runtime,
        terrain,
        particles,
        base: source_context,
        players,
        class3,
        families,
    };
    if stage.run(&mut row)? {
        stage.publish(&row)?;
    }
    Ok(Effects::default())
}

pub(super) fn step_alliance_attack(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    source_context: source_target::Context<'_>,
    players: &mut source_target::Creature23PlayerFeedback<'_>,
    slot: u16,
    class3: &[u16],
    families: &[Vec<u16>; 29],
) -> Result<Effects, String> {
    let mut row = arena
        .entities
        .get(usize::from(slot))
        .cloned()
        .ok_or_else(|| format!("model23 Alliance attack lost slot{slot}"))?;
    let filtered_target = {
        let mut stage = Stage {
            arena,
            runtime,
            terrain,
            particles,
            base: source_context,
            players,
            class3,
            families,
        };
        stage.movement(&mut row)?;
        stage.blink(&mut row, false)?;
        let ticks = remaining(&row).wrapping_sub(1);
        set_remaining(&mut row, ticks);
        let target = row.source_word_96();
        stage.publish(&row)?;
        if ticks <= 0 || target == 0 {
            None
        } else {
            let parent_slot = word(&row, 44);
            if parent_slot == 0 {
                Some(target)
            } else {
                let parent = stage.packet(parent_slot)?;
                let inherited = if parent.source_word_96() != 0 {
                    parent.source_word_96()
                } else {
                    parent.word_26()
                };
                (target != parent_slot && (inherited == 0 || inherited == target)).then_some(target)
            }
        }
    };
    super::mc2::attack_after_move(
        arena,
        runtime,
        terrain,
        particles,
        source_context,
        players,
        slot,
        class3,
        families,
        filtered_target,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign::Mc2Simulation;
    use crate::native_world::{Mc2DispatchEvent, RuntimeRelation, area_damage};

    fn qualified_constructor_counters() -> &'static [u8; 29] {
        static COUNTERS: std::sync::OnceLock<[u8; 29]> = std::sync::OnceLock::new();
        COUNTERS.get_or_init(|| {
            let package = std::env::var_os("MC_CREATURE23_LEVEL18_PACKAGE").map_or_else(
                || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/all-spells-20261104-integration/current-owner-package/level18"),
                std::path::PathBuf::from,
            );
            let owner = crate::campaign::CampaignSession::open(
                package, crate::campaign::spec::CampaignSelection::ordinary(18).unwrap(),
            ).expect("actual whole Level18 source runtime metadata");
            *owner.simulation().runtime().constructor_counters()
                .expect("qualified owner exports all29 source constructor counters")
        })
    }

    fn configured(mode: u8) -> (Mc2Simulation, u16) {
        // Explicit source StageVar configuration, not an authored/default
        // producer claim. Inspected ordinary14/17/18/19 have no model23 route.
        let mut session = crate::native_world::tests::level_two_campaign_session();
        let live = &mut session.session.state.live;
        if live.core.runtime.constructor_counters().is_none() {
            // The old compact L2 capture omits this source-init prerequisite.
            // Configure it from the real decoded owner input, not guessed
            // zeros or a constructor fallback; all other L2 bytes remain.
            assert_eq!(
                live.core.runtime.bytes.len(),
                crate::native_world::RUNTIME_BYTES
            );
            let mut bytes = live.core.runtime.bytes.to_vec();
            bytes.extend_from_slice(qualified_constructor_counters());
            live.core.runtime.bytes = bytes.into_boxed_slice();
        }
        let counter_before = live.core.runtime.constructor_counters().unwrap()[23];
        let position = live.core.state.position;
        live.core.runtime.stage_variables[1].kind = i8::try_from(mode).unwrap();
        live.core.runtime.stage_variables[1].flags = 0;
        live.core.runtime.stage_variables[1].relation =
            RuntimeRelation::AxisScalar(u32::from(position.x) | (u32::from(position.y) << 16));
        let slot = super::super::mc2::allocate_authored(
            &mut live.economy,
            &mut live.core.runtime,
            &mut live.terrain,
            &session.session.definition.presentation.particles,
            position,
            RULES.max_life,
            Some(1),
            65023,
        )
        .unwrap()
        .unwrap();
        let row = &mut live.economy.entities[usize::from(slot)];
        assert_eq!((row.action, row.record[77]), (191, mode));
        assert_eq!(row.phase, counter_before);
        assert_eq!(
            live.core.runtime.constructor_counters().unwrap()[23],
            counter_before.wrapping_add(1)
        );
        // Declared initial route/lifetime/cadence inputs only. Later visits
        // consume these fields naturally; checks do not force transitions.
        put(row, 44, live.core.state.entity_index);
        set_remaining(row, 64);
        put(row, 52, 128);
        row.set_phase(1);
        row.set_control_flags(row.control_flags | 0x800);
        (session, slot)
    }

    fn try_visit(
        session: &mut Mc2Simulation,
        slot: u16,
        families: &[Vec<u16>; 29],
        class3: &[u16],
    ) -> Result<Effects, String> {
        let live = &mut session.session.state.live;
        let particles = &session.session.definition.presentation.particles;
        let base = source_target::Context {
            player: Some((
                live.core.state.entity_index,
                live.core.state.position,
                live.player_life,
            )),
            player_orientation: Some((live.core.state.yaw, live.core.state.roll_accumulator)),
            particles: Some(particles),
            map_type: Some(live.terrain.map_type()),
            ..source_target::Context::default()
        };
        let mut feedback = source_target::Creature23PlayerFeedback {
            shared: &mut live.shared_spells,
            core_players: &mut live.core.players,
            fire: live.core.context.spell_fire.as_mut(),
            local_state: &mut live.core.state,
            domains: Some(
                area_damage::ActorDomains::new(&mut live.goats, &mut live.ordinary)
                    .with_particles(particles),
            ),
        };
        super::super::mc2::step_row(
            &mut live.economy,
            &mut live.core.runtime,
            &mut live.terrain,
            particles,
            base,
            &mut feedback,
            slot,
            class3,
            families,
            &[],
        )
    }

    fn visit(session: &mut Mc2Simulation, slot: u16, families: &[Vec<u16>; 29], class3: &[u16]) {
        try_visit(session, slot, families, class3).unwrap();
    }

    #[test]
    fn configured_stage_modes_keep_literal_returns_and_source_movement_admission() {
        for mode in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 15, 18] {
            let (mut session, slot) = configured(mode);
            if matches!(mode, 1 | 5) {
                // Off-phase no-hit visits do not read StageVars either.
                session.session.state.live.economy.entities[usize::from(slot)].record[76] = 0;
            }
            let before = session.entities()[usize::from(slot)].clone();
            visit(
                &mut session,
                slot,
                &std::array::from_fn(|_| Vec::new()),
                &[],
            );
            let after = &session.entities()[usize::from(slot)];
            assert_eq!(
                after.phase, before.phase,
                "outer scheduler alone advances byte3E"
            );
            assert_eq!(
                after.entity_rng, before.entity_rng,
                "phase1 admits no source jitter"
            );
            assert_eq!(
                after.action,
                if mode == 10 { 186 } else { 191 },
                "mode{mode}"
            );
            assert_eq!(
                after.control_flags & 0x800,
                if matches!(mode, 1..=9) { 0 } else { 0x800 },
                "mode{mode}"
            );
            assert_eq!(after.life, before.life);
            assert_eq!(remaining(after), remaining(&before));
        }
    }

    #[test]
    fn configured_stage_contact_consumes_mailbox_before_movement_and_retains_payload() {
        for (mode, index, phase) in [(5, 1, 1), (1, 0, 1), (5, 0, 1), (3, 0, 0), (4, 0, 0)] {
            for lethal in [false, true] {
                let (mut session, slot) = configured(mode);
                let row = &mut session.session.state.live.economy.entities[usize::from(slot)];
                row.record[76] = index;
                row.set_phase(phase);
                let owner = session.state.entity_index;
                let amount = if lethal {
                    (RULES.max_life + 1) as u16
                } else {
                    100
                };
                area_damage::contact(
                    &mut session.session.state.live.economy,
                    None,
                    slot,
                    amount,
                    owner,
                )
                .unwrap();
                assert_eq!(session.entities()[usize::from(slot)].action, 191);
                let result = try_visit(
                    &mut session,
                    slot,
                    &std::array::from_fn(|_| Vec::new()),
                    &[],
                );
                if matches!(mode, 3 | 4) {
                    assert!(
                        result.unwrap_err().contains("stage descriptor 0 is absent"),
                        "post-helper acquisition reaches its descriptor after committing damage"
                    );
                } else {
                    result.unwrap();
                }
                let row = &session.entities()[usize::from(slot)];
                assert_eq!(row.life, RULES.max_life - i32::from(amount));
                assert_eq!(row.action, if lethal { 188 } else { 186 });
                assert_eq!(row.word_26(), owner);
                assert_eq!(word(row, 102), 0);
                assert_eq!(
                    i32::from_le_bytes(row.record[98..102].try_into().unwrap()),
                    i32::from(amount)
                );
                assert_eq!(row.control_flags & 0x800, 0x800, "hit branches do not move");
                if lethal {
                    assert_eq!(row.word_24(), owner);
                }
            }
        }
    }

    #[test]
    fn configured_mode12_reads_typed_parent_and_publishes_source_follow_proposal() {
        let (mut session, slot) = configured(12);
        let expected = {
            let live = &mut session.session.state.live;
            let parent = live
                .economy
                .economy
                .build_castle(
                    Position {
                        x: 12345,
                        y: 23456,
                        z: 0,
                    },
                    live.core.state.entity_index,
                    &live.terrain,
                )
                .unwrap();
            let castle = live.economy.economy.castle_entry_mut(parent).unwrap();
            castle.source_yaw = 321;
            castle.source_roll = -456;
            put(&mut live.economy.entities[usize::from(slot)], 44, parent);
            live.economy.retire_previous_source_projection(parent);
            let raw = &mut live.economy.entities[usize::from(parent)];
            raw.set_life(-1);
            raw.set_position(Position::default());
            raw.set_yaw(-99);
            raw.set_source_roll(88);
            source_target::status_parent(&live.economy, source_target::Context::default(), parent)
                .unwrap()
                .unwrap()
        };
        assert!(expected.life >= 0);
        visit(
            &mut session,
            slot,
            &std::array::from_fn(|_| Vec::new()),
            &[],
        );
        let row = &session.entities()[usize::from(slot)];
        assert_eq!(
            row.position,
            Position {
                z: expected.position.z.max(0),
                ..expected.position
            }
        );
        assert_eq!((row.yaw, row.source_roll()), (321, -456));
        assert_eq!(row.control_flags & 0x400, 0);
        assert_eq!(
            session.canonical_terrain().source_predicted_position(),
            row.position
        );
        assert_eq!(
            row.control_flags & 0x800,
            0x800,
            "follow copy does not run1B8C0"
        );
    }

    #[test]
    fn configured_mode13_expiry_runs_scheduler_effects_frozen_membership_and_reclaim() {
        let (mut session, slot) = configured(13);
        set_remaining(
            &mut session.session.state.live.economy.entities[usize::from(slot)],
            1,
        );
        let handle = session.source_arena().handle(slot).unwrap();
        let phase = session.entities()[usize::from(slot)].phase;
        let before = session
            .entities()
            .iter()
            .filter(|row| row.active && row.class == 10)
            .map(|row| session.source_arena().handle(row.index).unwrap())
            .collect::<Vec<_>>();
        let (frozen, _) = session
            .step_level2_source_pass_with_player(
                1,
                crate::LatchedCommand::default(),
                crate::FlightTuning::CLASSIC,
                &mut Vec::new(),
            )
            .unwrap();
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "{:?}",
            session.dispatch_events()
        );
        let row = &session.entities()[usize::from(slot)];
        assert_eq!(remaining(row), 0);
        assert_ne!(row.control_flags & 0x400, 0);
        assert_eq!(row.phase, phase.wrapping_add(1));
        assert!(session.source_arena().resolve(handle).is_some());
        assert!(
            !session
                .creature_identity_for_source_slot(slot)
                .unwrap()
                .living
        );
        assert!(frozen[23], "source list remains frozen through hide");
        assert!(
            !session.source_arena().rebuilt_class5_model_presence()[23],
            "a fresh current list already excludes the hidden root"
        );
        for model in [73, 0] {
            assert!(
                session.entities().iter().any(|row| row.active
                    && (row.class, row.model) == (10, model)
                    && session
                        .source_arena()
                        .handle(row.index)
                        .is_some_and(|handle| !before.contains(&handle))),
                "actual10/{model} constructor"
            );
        }
        let (following, _) = session
            .step_level2_source_pass_with_player(
                2,
                crate::LatchedCommand::default(),
                crate::FlightTuning::CLASSIC,
                &mut Vec::new(),
            )
            .unwrap();
        assert!(session.source_arena().resolve(handle).is_none());
        assert!(!following[23]);
        assert!(!session.source_arena().rebuilt_class5_model_presence()[23]);
    }

    #[test]
    fn configured_mode16_uses_reclaimed_parents_current_generation_then_expires() {
        use crate::native_world::lightning::{self, Product};

        // A configured mode16 relation exercises the fixed-address source
        // helper; it does not assert an authored model23 summon producer.
        let (mut session, slot) = configured(16);
        // The configured summon route consumes an actual damage mailbox
        // before source6D8B0. This is a callback/owner boundary control, not
        // evidence of an authored model23 Army producer.
        let mut retaliation = session.clone();
        let owner = retaliation.state.entity_index;
        let experience_before = retaliation.shared_spells().spell_experience()[19];
        {
            let live = &mut retaliation.session.state.live;
            for player in live.shared_spells.players.iter_mut()
                .chain(live.core.players.iter_mut())
                .chain(std::iter::once(&mut live.shared_spells.local_player))
                .filter(|player| player.entity_index == owner)
            {
                player.runtime_spell_experience[2] = 12;
                player.record[172..176].copy_from_slice(&12_i32.to_le_bytes());
            }
            live.shared_spells.spell_experience[2] = 12;
            live.core.context.spell_fire.as_mut().unwrap().spell_experience[2] = 12;
            let attacker = live.shared_spells.players.iter()
                .find(|player| player.entity_index != owner).unwrap().entity_index;
            area_damage::contact(&mut live.economy, None, slot, 1, attacker).unwrap();
        }
        visit(&mut retaliation, slot, &std::array::from_fn(|_| Vec::new()), &[]);
        assert_eq!(retaliation.entities()[usize::from(slot)].action, 186);
        let live = &retaliation.session.state.live;
        assert_eq!(live.shared_spells.spell_experience()[19], experience_before + 1);
        assert_eq!(live.shared_spells.spell_experience()[2], 7);
        for player in live.shared_spells.players.iter().chain(live.core.players.iter())
            .chain(std::iter::once(&live.shared_spells.local_player))
            .filter(|player| player.entity_index == owner)
        {
            assert_eq!(player.runtime_spell_experience[19], experience_before + 1);
            assert_eq!(player.runtime_spell_experience[2], 7);
        }
        let fire = live.core.context.spell_fire.as_ref().unwrap();
        assert_eq!(fire.spell_experience[19], experience_before + 1);
        assert_eq!(fire.spell_experience[2], 7);
        let child = session.source_arena().handle(slot).unwrap();
        let particles = session.session.definition.presentation.particles.clone();
        let (parent_slot, parent) = {
            let live = &mut session.session.state.live;
            let position = live.economy.entities[usize::from(slot)].position;
            let parent_slot = lightning::construct(
                &mut live.economy, &mut live.terrain, &particles, Product::Ray, position,
            ).unwrap().expect("actual parent NewEvent");
            let parent = live.economy.handle(parent_slot).unwrap();
            assert!(live.economy.entities[usize::from(parent_slot)].life > 0);
            put(&mut live.economy.entities[usize::from(slot)], 44, parent_slot);
            assert!(live.economy.bind_carried_creature_parent(child, parent));
            live.economy.entities[usize::from(parent_slot)].disable_drawing();
            assert!(live.economy.resolve(parent).is_some(), "hide retains the parent");
            // Actual source49F90 reclaims a previously hidden current row;
            // it rebuilds the free stack, without fabricating a free slot.
            live.economy.rebuild_source_arena_with_terrain(&mut live.terrain, &[]).unwrap();
            assert!(live.economy.resolve(parent).is_none());
            (parent_slot, parent)
        };
        let replacement = {
            let live = &mut session.session.state.live;
            let position = live.economy.entities[usize::from(slot)].position;
            let actual = lightning::construct(
                &mut live.economy, &mut live.terrain, &particles, Product::Ray, position,
            ).unwrap().expect("actual replacement NewEvent");
            assert_eq!(actual, parent_slot, "the genuine free stack selects the reclaimed address");
            let replacement = live.economy.handle(actual).unwrap();
            assert_ne!(replacement, parent);
            assert!(live.economy.entities[usize::from(actual)].life > 0);
            assert_eq!(live.economy.creature_parent_binding(child), Some(parent));
            assert_eq!(live.economy.creature_parent(child), None);
            replacement
        };

        // No target and phase1 keep 1E580 on its source follow path. The
        // positive current parent must win over the stale provenance handle.
        let before = remaining(&session.entities()[usize::from(slot)]);
        visit(&mut session, slot, &std::array::from_fn(|_| Vec::new()), &[]);
        let row = &session.entities()[usize::from(slot)];
        assert_eq!((row.action, row.record[77]), (191, 16));
        assert_eq!(remaining(row), before.wrapping_sub(4));
        assert!(!row.drawing_disabled());
        assert_eq!(session.source_arena().handle(parent_slot), Some(replacement));
        assert!(session.source_arena().resolve(child).is_some());

        let before_impacts = session.entities().iter()
            .filter(|row| row.active && (row.class, row.model) == (10, 0))
            .filter_map(|row| session.source_arena().handle(row.index))
            .collect::<Vec<_>>();
        session.session.state.live.economy.entities[usize::from(parent_slot)].disable_drawing();
        let phase = session.entities()[usize::from(slot)].phase;
        let (frozen, _) = session.step_level2_source_pass_with_player(
            1, crate::LatchedCommand::default(), crate::FlightTuning::CLASSIC, &mut Vec::new(),
        ).unwrap();
        assert!(session.dispatch_events().iter().all(|event|
            !matches!(event, Mc2DispatchEvent::Error { .. })), "{:?}", session.dispatch_events());
        let row = &session.entities()[usize::from(slot)];
        assert_eq!(remaining(row), 0);
        assert!(row.drawing_disabled());
        assert_eq!(row.phase, phase.wrapping_add(1));
        assert!(session.source_arena().resolve(child).is_some(), "hide retains the original child generation");
        assert!(frozen[23], "the pass-start membership retains the child through expiry");
        assert!(session.entities().iter().any(|row| row.active && (row.class, row.model) == (10, 0)
            && session.source_arena().handle(row.index).is_some_and(|h| !before_impacts.contains(&h))),
            "mode16 creates the actual Impact");
        let (following, _) = session.step_level2_source_pass_with_player(
            2, crate::LatchedCommand::default(), crate::FlightTuning::CLASSIC, &mut Vec::new(),
        ).unwrap();
        assert!(session.source_arena().resolve(child).is_none());
        assert!(!following[23]);
    }

    #[test]
    fn configured_mode14_blink_commits_current_external_goat_flags() {
        let (mut session, slot) = configured(14);
        let goat = {
            let live = &mut session.session.state.live;
            let goat = live
                .goats
                .goats()
                .iter()
                .find(|goat| goat.active)
                .unwrap()
                .entity_index;
            assert_eq!(
                live.goats
                    .goats()
                    .iter()
                    .find(|row| row.entity_index == goat)
                    .unwrap()
                    .control_flags
                    & 0x02000000,
                0
            );
            let row = &mut live.economy.entities[usize::from(slot)];
            row.set_phase(2);
            set_remaining(row, 16);
            put(row, 52, 64);
            put(row, 56, goat);
            let shadow = &mut live.economy.entities[usize::from(goat)];
            shadow.set_control_flags(shadow.control_flags | 0x02000000);
            put(shadow, 56, 0);
            goat
        };
        visit(
            &mut session,
            slot,
            &std::array::from_fn(|_| Vec::new()),
            &[],
        );
        let live = &session.session.state.live;
        assert_eq!(remaining(&live.economy.entities[usize::from(slot)]), 15);
        let actual = live
            .goats
            .goats()
            .iter()
            .find(|row| row.entity_index == goat)
            .unwrap();
        assert_eq!(actual.control_flags & 0x06000000, 0x04000000);
        let current = source_target::read(
            &live.economy,
            source_target::Context::from_ordinary(&live.goats, &live.ordinary),
            goat,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            current.control_flags & 0x06000000,
            0x04000000,
            "actual current-source consumers read the typed flag owner"
        );
    }
}
