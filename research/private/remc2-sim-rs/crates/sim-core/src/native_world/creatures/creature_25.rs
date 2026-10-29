//! Cymmerian (class 5/model 25) source callbacks from pinned remc2.
//!
//! The source row is the authority. This leaf owns callback-local decisions;
//! the campaign adapter stores the row and materializes returned source events.

use crate::level_definition::{CYMMERIAN_CLASS_PROPERTIES, CymmerianClassProperties};
use crate::native_world::{
    ENTITY_DRAW_DISABLED_BIT, EntitySnapshot, SessionTerrain, level2_next_rng16,
};
use crate::{FlightWorld, Position, angle_distance, heading_for_positions};

pub(crate) mod mc2;
mod stage;
#[cfg(test)]
pub(crate) mod tests;

pub(crate) const SOURCE_CLASS: u8 = 5;
pub(crate) const SOURCE_MODEL: u8 = 25;
pub(crate) const DESCRIPTOR: u16 = 92;
pub(crate) const SPLIT_DESCRIPTOR: u16 = 95;
pub(crate) const PARTICLE: u16 = 290;
pub(crate) const SPLIT_PARTICLE: u16 = 314;

/// The rule catalog is owned by sim-definition; this alias keeps the leaf
/// bound to the typed model definition instead of repeating constructor data.
pub(crate) const RULES: CymmerianClassProperties = CYMMERIAN_CLASS_PROPERTIES;

// These values are the descriptor-92 fields consumed by the source generic
// action handlers (`sub_1BF90` / `sub_1C310`).
const SOURCE_PROJECTILE_DAMAGE: u16 = RULES.projectile_damage;
const SOURCE_CASTLE_DAMAGE: i32 = 60;

fn is_split_body(actor: &Actor<'_>) -> bool {
    actor.row.parameter_relation == Some(SPLIT_DESCRIPTOR)
}

fn scan_period(actor: &Actor<'_>) -> u8 {
    if is_split_body(actor) {
        RULES.split_scan_period
    } else {
        RULES.scan_period
    }
}

fn target_range(actor: &Actor<'_>) -> u32 {
    if is_split_body(actor) {
        RULES.split_target_range
    } else {
        RULES.target_range
    }
}

fn target_half_angle(actor: &Actor<'_>) -> i16 {
    if is_split_body(actor) {
        RULES.split_target_half_angle
    } else {
        RULES.target_half_angle
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Target {
    pub slot: u16,
    pub source_id: u16,
    pub class: u8,
    pub model: u8,
    pub life: i32,
    pub flags: u32,
    pub position: Position,
    pub yaw: Option<i16>,
    pub roll: Option<i16>,
    pub pitch_extent: Option<i16>,
    pub roll_extent: Option<i16>,
    pub attacker: Option<u16>,
    pub target: Option<u16>,
    pub stage_index: Option<u8>,
    pub stage_mode: Option<u8>,
    pub parent: Option<u16>,
    pub stage_relation: Option<u16>,
    pub next: Option<u16>,
    pub castle_slot: u16,
}

impl Target {
    fn living(self) -> bool {
        self.life >= 0 && self.flags & ENTITY_DRAW_DISABLED_BIT == 0
    }

    fn required<T>(self, value: Option<T>, field: &str) -> Result<T, String> {
        value.ok_or_else(|| {
            format!(
                "Cymmerian source target {} lacks current {field}",
                self.slot
            )
        })
    }
}

pub(crate) struct Actor<'a> {
    pub row: &'a mut EntitySnapshot,
}

impl Actor<'_> {
    fn word(&self, at: usize) -> u16 {
        word(self.row, at)
    }

    fn put(&mut self, at: usize, value: u16) {
        put(self.row, at, value);
    }

    fn owner(&self) -> u16 {
        self.word(40)
    }

    fn target(&self) -> u16 {
        self.word(154)
    }

    fn set_target(&mut self, slot: u16) {
        self.put(154, slot);
    }

    fn state(&self) -> u8 {
        self.row.record[74]
    }

    fn set_state(&mut self, state: u8) {
        self.row.record[74] = state;
    }

    fn timer(&self) -> i32 {
        self.row.source_dword_10()
    }

    fn set_timer(&mut self, value: i32) {
        self.row.set_source_dword_10(value);
    }

    fn set_action(&mut self, action: u8) {
        self.row.set_source_action(action);
    }
}

pub(crate) struct Context<'a> {
    pub terrain: &'a SessionTerrain,
    pub targets: &'a [Target],
    /// Frozen source membership in ascending source-slot order.
    pub class3: &'a [u16],
    /// Frozen model25 membership used by the source same-model avoidance scan.
    pub model25: &'a [u16],
    pub reader: Option<(
        &'a crate::campaign::source_arena::SourceArena,
        crate::native_world::source_target::Context<'a>,
    )>,
    pub read_error: std::cell::Cell<Option<u16>>,
}

impl Context<'_> {
    fn target(&self, slot: u16) -> Option<Target> {
        if slot == 0 {
            return None;
        }
        if let Some((arena, context)) = self.reader {
            return match mc2::target_at(arena, context, slot) {
                Ok(target) => target,
                Err(_) => {
                    self.read_error.set(Some(slot));
                    None
                }
            };
        }
        (slot != 0)
            .then(|| {
                self.targets
                    .iter()
                    .find(|target| target.slot == slot)
                    .copied()
            })
            .flatten()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceAttack {
    pub owner: u16,
    pub owner_id: u16,
    pub target: u16,
    pub target_class: u8,
    pub target_model: u8,
    pub constructor_position: Position,
    pub position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub parameter: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CastleHit {
    pub player_slot: u16,
    pub castle_slot: u16,
    pub damage: i32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Effects {
    /// Source class-9/model-0 constructor requested by `sub_1CC20`.
    pub attack: Option<SourceAttack>,
    /// Source `sub_11900(..., 0, 60)` request from Cymmerian castle contact.
    pub castle_hit: Option<CastleHit>,
    /// Action 204 on a byte46-nonzero child walks the source child chain.
    pub prekill: bool,
    /// Action 204 on an ordinary root splits into three Cymmerians in `mc2`.
    pub split: bool,
    /// Action 205's kill path is different for summoned children and ordinary
    /// roots; the adapter preserves the source byte46 distinction.
    pub kill: bool,
    pub kill_credit: Option<u16>,
    /// Action 207 delegates to the shared StageVar consumer when byte46 is 0.
    pub stage_callback: bool,
    pub visual: Option<u16>,
    pub speed_changed: bool,
    pub population_increments: u32,
    pub population_decrements: u32,
    pub linked_blink_flags: Vec<(u16, u32)>,
    pub feedback: Option<(u16, u8)>,
    pub earlier_feedback: Option<(u16, u8)>,
}

fn word(row: &EntitySnapshot, at: usize) -> u16 {
    u16::from_le_bytes([row.record[at], row.record[at + 1]])
}

fn put(row: &mut EntitySnapshot, at: usize, value: u16) {
    row.record[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn xy_squared(a: Position, b: Position) -> u32 {
    let dx = i32::from(b.x.wrapping_sub(a.x) as i16);
    let dy = i32::from(b.y.wrapping_sub(a.y) as i16);
    dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32
}

fn distance(a: Position, b: Position) -> u32 {
    crate::legacy_sqrt(crate::native_world::level2_wrapping_source_distance_3d(
        a, b,
    ))
}

fn consume(actor: &mut Actor<'_>, ctx: &Context<'_>) -> Result<u8, String> {
    let mut damage = i32::from_le_bytes(
        actor.row.record[98..102]
            .try_into()
            .expect("source damage dword has fixed width"),
    );
    let mut pending = actor.word(102);
    let mut attacker = actor.word(42);
    let mut life = actor.row.life;
    let mut hit = u8::from(
        crate::combat::actors::consume_damage(&mut life, &mut damage, &mut pending, &mut attacker)
            .is_some(),
    );

    // `sub_28860`, `sub_1BF90`, and `sub_1C310` all compare the linked child
    // chain's life before committing the parent result.
    let mut next = actor.word(56);
    for _ in 0..1000 {
        let Some(child) = ctx.target(next) else { break };
        if child.life < life
            && crate::combat::actors::inherit_linked_life(
                &mut life,
                &mut attacker,
                child.life,
                child.required(child.attacker, "attacker")?,
            )
        {
            hit = 1;
            break;
        }
        next = child.required(child.next, "linked child")?;
    }

    actor.row.set_life(life);
    actor.row.record[98..102].copy_from_slice(&damage.to_le_bytes());
    actor.put(102, pending);
    actor.put(42, attacker);
    if life < 0 {
        actor.put(40, attacker);
        Ok(2)
    } else {
        Ok(hit)
    }
}

fn jitter(actor: &mut Actor<'_>) {
    let first = level2_next_rng16(actor.row.entity_rng);
    let next = level2_next_rng16(first);
    let direction = 2 * (i32::from(first % 157) / 79) - 1;
    let delta = (i32::from(next & 0xff) + 85) * direction;
    actor.row.set_entity_rng(next);
    actor
        .row
        .set_source_roll(actor.row.source_roll().wrapping_add(delta as i16) & 2047);
}

fn move_actor(actor: &mut Actor<'_>, ctx: &Context<'_>) {
    if actor.row.control_flags & 0x800 != 0 {
        actor
            .row
            .set_control_flags(actor.row.control_flags & !0x800);
        return;
    }
    let motion = crate::ground_creature_motion::step_ground_creature_motion(
        actor.row.position,
        actor.row.yaw,
        actor.row.source_roll(),
        actor.row.actual_speed,
        ctx.terrain,
        crate::ground_creature_motion::GroundCreatureMotionConfig {
            half_width: if is_split_body(actor) {
                RULES.split_half_extent
            } else {
                RULES.collision_half_width
            },
            half_height: if is_split_body(actor) {
                RULES.split_half_extent
            } else {
                RULES.collision_half_height
            },
            ground_clearance: RULES.ground_clearance,
            descent: if is_split_body(actor) {
                RULES.split_descent
            } else {
                RULES.descent
            },
            turn_limit: if is_split_body(actor) {
                RULES.split_turn_limit
            } else {
                RULES.turn_limit
            },
            terrain_mask: RULES.terrain_mask,
            maximum_tile_slope: if is_split_body(actor) {
                RULES.split_maximum_slope
            } else {
                RULES.maximum_slope
            },
        },
    );
    actor.row.set_position(motion.position);
    actor.row.set_yaw(motion.yaw);
    if motion.movement_blocked {
        actor
            .row
            .set_control_flags(actor.row.control_flags | 0x40000);
    } else {
        actor
            .row
            .set_control_flags(actor.row.control_flags & !0x40000);
    }
    if motion.killed && RULES.behavior_flags & 1 != 0 {
        actor.row.set_life(-1);
    }
}

fn update_water(actor: &mut Actor<'_>, ctx: &Context<'_>, effects: &mut Effects) {
    let visual = actor.row.particle_parameter_index;
    if ctx.terrain.material_mask(actor.row.position) == crate::WATER_AMBIENCE_MASK {
        if visual == Some(SPLIT_PARTICLE)
            && ctx.terrain.ground_height(actor.row.position) < actor.row.position.z
        {
            effects.visual = Some(313);
            put(actor.row, 136, RULES.minimum_speed as u16);
            effects.speed_changed = true;
        } else if visual != Some(SPLIT_PARTICLE) {
            effects.visual = Some(SPLIT_PARTICLE);
            put(actor.row, 136, RULES.split_minimum_speed as u16);
            effects.speed_changed = true;
        }
    } else if visual != Some(313) {
        effects.visual = Some(313);
        put(actor.row, 136, RULES.minimum_speed as u16);
        effects.speed_changed = true;
    }
}

fn nearest_target(actor: &Actor<'_>, ctx: &Context<'_>) -> Option<Target> {
    let range = target_range(actor);
    let half_angle = target_half_angle(actor);
    let range_squared = range * range;
    let mut best = None;
    let mut shortest = u32::MAX;
    for target in ctx
        .class3
        .iter()
        .filter_map(|slot| ctx.target(*slot))
        .filter(|target| target.flags & 0x20 == 0)
    {
        let distance = xy_squared(actor.row.position, target.position);
        if distance <= range_squared
            && distance < shortest
            && angle_distance(
                i32::from(actor.row.yaw),
                heading_for_positions(actor.row.position, target.position),
            ) < half_angle
        {
            shortest = distance;
            best = Some(target);
        }
    }
    best
}

fn seek(actor: &mut Actor<'_>, ctx: &Context<'_>) -> Result<(), String> {
    match consume(actor, ctx)? {
        2 => {
            actor.set_action(204);
            return Ok(());
        }
        1 => {
            actor.set_target(actor.word(42));
            actor.set_action(202);
            return Ok(());
        }
        _ => {}
    }
    move_actor(actor, ctx);
    if actor.row.phase % scan_period(actor) == 0 {
        jitter(actor);
        if actor.row.record[61] != 0
            && let Some(target) = nearest_target(actor, ctx)
        {
            actor.set_target(target.slot);
            actor.set_action(202);
        }
    }
    Ok(())
}

fn pursue(actor: &mut Actor<'_>, ctx: &Context<'_>, effects: &mut Effects) -> Result<(), String> {
    match consume(actor, ctx)? {
        2 => {
            actor.set_action(204);
            return Ok(());
        }
        1 => {
            actor.set_target(actor.word(42));
            return Ok(());
        }
        _ => {}
    }
    move_actor(actor, ctx);
    let target = ctx
        .target(actor.target())
        .filter(|target| target.living() && target.flags & 0x400 == 0);
    let Some(target) = stage::alliance_target(actor, ctx, effects, target)? else {
        actor.set_action(201);
        return Ok(());
    };
    if actor.row.phase & 3 == 0 {
        actor
            .row
            .set_source_roll(heading_for_positions(actor.row.position, target.position) as i16);
        for slot in ctx.model25 {
            let Some(peer) = ctx.target(*slot) else {
                continue;
            };
            let dx = i32::from(peer.position.x.wrapping_sub(actor.row.position.x) as i16).abs();
            let dy = i32::from(peer.position.y.wrapping_sub(actor.row.position.y) as i16).abs();
            if peer.source_id != actor.row.source_id
                && dx < i32::from(actor.row.array_pitch())
                && dy < i32::from(actor.row.array_pitch())
            {
                actor.row.set_source_roll(
                    heading_for_positions(peer.position, actor.row.position) as i16
                );
                break;
            }
        }
    }
    if actor.row.phase % scan_period(actor) != 0 {
        return Ok(());
    }
    if distance(actor.row.position, target.position) >= target_range(actor) {
        actor.set_action(201);
        return Ok(());
    }
    let (yaw, pitch) =
        crate::level1_mission::projectile_guidance_angles(actor.row.position, target.position);
    effects.attack = Some(SourceAttack {
        owner: actor.row.index,
        owner_id: actor.row.source_id,
        target: target.slot,
        target_class: target.class,
        target_model: target.model,
        constructor_position: actor.row.position,
        position: Position {
            z: actor.row.position.z.wrapping_add(actor.row.array_fov()),
            ..actor.row.position
        },
        yaw,
        pitch,
        parameter: SOURCE_PROJECTILE_DAMAGE,
    });
    Ok(())
}

fn actor_collision(actor: &Actor<'_>, target: Target) -> Result<bool, String> {
    let x = i32::from(target.position.x.wrapping_sub(actor.row.position.x) as i16).abs();
    let y = i32::from(target.position.y.wrapping_sub(actor.row.position.y) as i16).abs();
    Ok(x < i32::from(actor.row.array_pitch())
        + i32::from(target.required(target.pitch_extent, "collision width")?)
        && y < i32::from(actor.row.array_roll())
            + i32::from(target.required(target.roll_extent, "collision depth")?))
}

fn special(actor: &mut Actor<'_>, ctx: &Context<'_>, effects: &mut Effects) -> Result<(), String> {
    let mut result = 0;
    if !matches!(actor.state(), 1 | 2) {
        let hit = consume(actor, ctx)?;
        if hit == 2 {
            result = 2;
        } else if hit == 1 {
            result = 1;
        }
    }
    // Pinned sub_28860 decrements subSpellIndex on every action200 visit and
    // selects action204 at zero. It is independent of dword_0x10, which the
    // byte46 state machine uses for its own timers.
    actor.row.subspell_index = actor.row.subspell_index.wrapping_sub(1);
    if actor.row.subspell_index == 0 {
        result = 2;
    }
    if result == 2 {
        actor.set_action(204);
        return Ok(());
    }

    let previous_state = actor.state();
    match previous_state {
        1 | 2 => {
            if previous_state == 1 {
                actor.set_timer(52);
                actor.set_state(2);
            }
            actor.row.set_life(actor.row.max_life);
            let timer = actor.timer().wrapping_sub(1);
            actor.set_timer(timer);
            if timer < 0 {
                actor.set_state(3);
            } else if timer > 13 {
                let roll = actor.row.source_roll() as u16;
                actor.row.set_source_roll(
                    ((roll & 0x00ff) | (((roll >> 8).wrapping_add(1) & 7) << 8)) as i16,
                );
            }
        }
        3 => {
            let player = ctx.target(actor.owner()).filter(|target| {
                ctx.class3.contains(&target.slot)
                    && target.class == 3
                    && matches!(target.model, 0 | 1)
            });
            match player {
                None => {
                    actor.set_state(8);
                    actor.set_timer(100);
                }
                Some(player) if player.castle_slot != 0 => {
                    actor.set_state(5);
                    actor.put(154, player.slot);
                }
                Some(_) => {
                    let rng = level2_next_rng16(actor.row.entity_rng);
                    actor.row.set_entity_rng(rng);
                    actor.set_state(4);
                    actor.set_timer(i32::from(rng % 100 + 100));
                }
            }
        }
        4 => {
            let timer = actor.timer().wrapping_sub(1);
            actor.set_timer(timer);
            if timer < 0 {
                actor.set_state(3);
            }
        }
        5 => {
            let Some(player) = ctx.target(actor.target()) else {
                actor.set_state(3);
                return Ok(());
            };
            let castle = ctx
                .target(player.castle_slot)
                .filter(|target| target.living());
            if let Some(castle) = castle {
                if actor.row.phase & 7 == 0 {
                    actor
                        .row
                        .set_source_roll(
                            heading_for_positions(actor.row.position, castle.position) as i16,
                        );
                    if actor_collision(actor, castle)? {
                        actor.set_state(6);
                    }
                }
            } else {
                actor.set_state(3);
            }
        }
        6 | 7 => {
            if previous_state == 6 {
                actor.set_state(7);
            }
            let Some(player) = ctx.target(actor.target()) else {
                actor.set_state(3);
                return Ok(());
            };
            if let Some(castle) = ctx
                .target(player.castle_slot)
                .filter(|target| target.living())
            {
                if actor_collision(actor, castle)? {
                    effects.castle_hit = Some(CastleHit {
                        player_slot: player.slot,
                        castle_slot: castle.slot,
                        damage: SOURCE_CASTLE_DAMAGE,
                    });
                } else {
                    actor.set_state(5);
                }
            } else {
                actor.set_state(3);
            }
        }
        8 => {
            let timer = actor.timer().wrapping_sub(1);
            actor.set_timer(timer);
            if timer < 0 {
                actor.set_action(204);
            }
        }
        _ => {}
    }

    if actor.row.phase & 7 == 0 {
        let first = level2_next_rng16(actor.row.entity_rng);
        let second = level2_next_rng16(first);
        let direction = 2 * (i32::from(first % 157) / 79) - 1;
        let delta = (i32::from(second % 0x17d) * direction) as i16;
        actor.row.set_entity_rng(second);
        actor
            .row
            .set_source_roll(actor.row.source_roll().wrapping_add(delta) & 2047);
    }
    move_actor(actor, ctx);
    update_water(actor, ctx, effects);
    if actor.state() != previous_state || effects.speed_changed {
        let minimum = i16::from_le_bytes([actor.row.record[136], actor.row.record[137]]);
        let bonus = if actor.state() == 2 {
            RULES.split_speed_bonus
        } else {
            0
        };
        actor.row.set_actual_speed(minimum.saturating_add(bonus));
        effects.speed_changed = true;
    }
    Ok(())
}

/// Runs one source callback. The ordinary scheduler still owns action enable
/// filtering, byte3E advancement, and next-pass slot reclamation.
pub(crate) fn step(actor: &mut Actor<'_>, ctx: &Context<'_>) -> Result<Effects, String> {
    let mut effects = Effects::default();
    match actor.row.action {
        200 => special(actor, ctx, &mut effects)?,
        201 => seek(actor, ctx)?,
        202 => pursue(actor, ctx, &mut effects)?,
        203 => {
            actor.set_action(201);
            if actor.row.life < 0 {
                actor.row.set_life(0);
            }
        }
        204 if actor.state() != 0 => effects.prekill = true,
        204 => {
            effects.split = true;
            actor.set_action(205);
            effects.population_increments = 3;
        }
        205 if actor.state() != 0 && actor.row.phase & 7 == 0 => {
            let owner = actor.owner();
            effects.kill_credit = (owner != 0).then_some(owner);
            effects.kill = true;
            effects.population_decrements = 1;
        }
        205 if actor.state() != 0 => {}
        205 => {
            actor.row.set_life(-1);
            actor.row.disable_drawing();
            effects.population_decrements = 1;
        }
        207 if actor.state() != 0 => stage::reset_to_special(actor),
        207 => effects.stage_callback = true,
        206 => {
            return Err("Cymmerian action206 has no implementation in pinned remc2".to_owned());
        }
        action => {
            return Err(format!("unsupported Cymmerian source action{action}"));
        }
    }
    Ok(effects)
}

pub(crate) fn construct_row(
    slot: u16,
    seed: u16,
    position: Position,
    phase: u8,
    particles: &[crate::native_world::ParticleParameter],
) -> Result<EntitySnapshot, String> {
    let mut row = crate::native_world::cave_features::new_event(slot, seed, position);
    row.class = SOURCE_CLASS;
    row.model = SOURCE_MODEL;
    row.record[67] = SOURCE_CLASS;
    row.record[68] = SOURCE_MODEL;
    row.set_source_action(201);
    row.set_max_life(RULES.max_life);
    row.set_life(RULES.max_life);
    row.set_mana(RULES.initial_mana);
    row.set_actual_speed(RULES.initial_speed);
    put(&mut row, 136, RULES.minimum_speed as u16);
    put(&mut row, 138, RULES.maximum_speed as u16);
    let rng = level2_next_rng16(seed);
    row.set_entity_rng(rng);
    let angle = (rng & 2047).wrapping_sub(1) as i16;
    row.set_yaw(angle);
    row.set_pitch(angle);
    row.set_source_roll(angle);
    row.set_fov(0);
    row.set_subspell_index(RULES.attack_damage);
    row.record[60] = 1;
    row.record[61] = 64;
    row.record[69] = 3;
    row.set_phase(phase);
    row.parameter_relation = Some(DESCRIPTOR);
    row.record[164..168].copy_from_slice(&(u32::from(DESCRIPTOR) + 1).to_le_bytes());
    crate::world::effects::bind_particle(&mut row, PARTICLE, particles)?;
    row.set_source_overlap_half_extents(RULES.collision_half_width, RULES.collision_half_width);
    put(&mut row, 92, RULES.collision_half_height as u16);
    Ok(row)
}
