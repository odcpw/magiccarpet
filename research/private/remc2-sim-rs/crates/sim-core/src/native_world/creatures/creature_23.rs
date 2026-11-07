//! Model23 flying mana predator behavior (MC2 actions184..191).
//!
//! The source arena row remains authoritative. Action186 commits Lightning
//! attacks, action187 captures ManaSpheres, and the leaf adapter owns their
//! product and reclaim effects.

use super::archer::ArcherStageRuntime as StageRuntime;
pub(crate) use crate::level_definition::CREATURE23_CLASS_PROPERTIES as RULES;
use crate::native_world::{
    ENTITY_DRAW_DISABLED_BIT, EntitySnapshot, SessionTerrain, level2_next_rng16,
};
use crate::{FlightWorld, Position, heading_for_positions};

pub(crate) mod mc2;
mod stage;
#[cfg(test)]
mod tests;

pub(crate) const SOURCE_MODEL: u8 = 23;
pub(crate) const DESCRIPTOR: u16 = 91;
const MANA_SPHERE_CLAIMED: u32 = 0x40;

/// The pinned callback table has no action190 handler. Keep it an explicit
/// rejection instead of advancing an unsupported row as a successful no-op.
pub(crate) const fn enabled(action: u8) -> bool {
    matches!(action, 184..=189 | 191)
}

pub(crate) struct Actor<'a> {
    pub row: &'a mut EntitySnapshot,
}

impl Actor<'_> {
    fn word(&self, at: usize) -> u16 {
        u16::from_le_bytes([self.row.record[at], self.row.record[at + 1]])
    }

    fn put(&mut self, at: usize, value: u16) {
        self.row.record[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn target(&self) -> u16 {
        self.word(154)
    }

    fn set_target(&mut self, slot: u16) {
        self.put(154, slot);
    }

    fn action(&self) -> u8 {
        self.row.action
    }

    fn set_action(&mut self, action: u8) {
        self.row.set_source_action(action);
    }

    fn mode(&self) -> u8 {
        self.row.record[74]
    }

    fn set_mode(&mut self, mode: u8) {
        self.row.set_source_byte_46(mode);
    }

    fn timer(&self) -> i32 {
        self.row.source_dword_10()
    }

    fn set_timer(&mut self, timer: i32) {
        self.row.set_source_dword_10(timer);
    }

    fn target_height(&self) -> i16 {
        self.word(48) as i16
    }

    fn set_target_height(&mut self, height: i16) {
        self.put(48, height as u16);
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Castle {
    pub position: Position,
    pub half_x: i16,
    pub half_y: i16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Target {
    pub slot: u16,
    pub source_id: u16,
    pub class: u8,
    pub model: u8,
    pub life: i32,
    pub mana: i32,
    pub control_flags: u32,
    pub position: Position,
    pub half_x: i16,
    pub half_y: i16,
    pub castle: Option<Castle>,
}

impl Target {
    fn living(self) -> bool {
        self.life >= 0 && self.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
    }

    fn source_candidate(self) -> bool {
        (matches!(self.model, 0 | 1) || (self.model == 3 && self.mana > 0))
            && self.castle.is_some_and(|castle| {
                !crate::native_world::level2_compare_axis_with_shift(
                    self.position,
                    self.half_x,
                    self.half_y,
                    castle.position,
                    castle.half_x,
                    castle.half_y,
                )
            })
    }
}

pub(crate) struct Context<'a> {
    pub terrain: &'a SessionTerrain,
    pub targets: &'a [Target],
    /// Resolved before borrowing mutable rows; source observes errors only
    /// after the movement that precedes its action186 target read.
    pub target_error: Option<&'a str>,
    /// Frozen mana-chain addresses resolved to their current model39 owners.
    pub mana_candidates: &'a [Result<Option<ManaCandidate>, String>],
    pub class3: &'a [u16],
    pub families: &'a [Vec<u16>],
    /// Frozen model23 addresses projected through their current source owners.
    /// Deferred errors are observed only if the source scan reaches that row.
    pub peers: &'a [Result<Option<crate::native_world::source_target::View>, String>],
    pub entities: &'a mut [EntitySnapshot],
    pub spheres: &'a mut [crate::level1_mission::ManaSphere],
    /// The source 1B8C0 all-blocked path decrements this at the move visit.
    pub population: Option<&'a mut u32>,
    pub runtime: &'a mut dyn StageRuntime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ManaCandidate {
    pub slot: u16,
    pub position: Position,
}

impl Context<'_> {
    fn target(&self, slot: u16) -> Option<Target> {
        (slot != 0)
            .then(|| {
                self.targets
                    .iter()
                    .find(|target| target.slot == slot)
                    .copied()
            })
            .flatten()
    }

    fn sphere(&self, slot: u16) -> Option<crate::level1_mission::ManaSphere> {
        (slot != 0)
            .then(|| {
                self.spheres
                    .iter()
                    .find(|sphere| sphere.active && sphere.source_entity_index == slot)
                    .copied()
            })
            .flatten()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LightningRequest {
    pub source_slot: u16,
    pub target_slot: u16,
    pub position: Position,
    pub damage: i32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Effects {
    /// Source sub_1D260: allocate class9/model9 first, then let its actual
    /// 10/23 consumer apply damage to this target.
    pub lightning: Option<LightningRequest>,
    pub kill_credit: Option<u16>,
    pub kill: bool,
    /// sub_27E00 runs sub_28110 after the real Lightning NewEvent boundary.
    pub combat_tail_pending: bool,
    /// Source applies its invalid/out-of-range v8 transition after that tail.
    pub attack_abandon: bool,
}

fn xy_distance(a: Position, b: Position) -> u32 {
    let dx = i32::from(a.x.wrapping_sub(b.x) as i16);
    let dy = i32::from(a.y.wrapping_sub(b.y) as i16);
    crate::legacy_sqrt(dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32)
}

// Despite its name, remc2's EuclideanDistXY_584D0 returns the squared
// wrapped XY distance. Selecting by the square avoids square-root ties.
fn xy_distance_squared(a: Position, b: Position) -> u32 {
    let dx = i32::from(a.x.wrapping_sub(b.x) as i16);
    let dy = i32::from(a.y.wrapping_sub(b.y) as i16);
    dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32
}

fn distance(a: Position, b: Position) -> u32 {
    crate::legacy_sqrt(crate::native_world::level2_wrapping_source_distance_3d(
        a, b,
    ))
}

fn living_sphere(sphere: crate::level1_mission::ManaSphere) -> bool {
    sphere.active
        && sphere.life >= 0
        && !sphere.pending_delete
        && sphere.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
}

fn nearest_sphere(
    actor: &Actor<'_>,
    ctx: &Context<'_>,
) -> Result<Option<ManaCandidate>, String> {
    let mut best = None;
    let mut shortest = RULES.mana_sphere_range;
    // sub_28000 checks model only on the frozen list's current occupants;
    // sub_28420 validates life, hide and class on the later target visit.
    for read in ctx.mana_candidates {
        let Some(sphere) = read.as_ref().map_err(Clone::clone)? else { continue };
        let d = xy_distance_squared(actor.row.position, sphere.position);
        if d < shortest {
            shortest = d;
            best = Some(*sphere);
        }
    }
    Ok(best)
}

fn nearest_victim(actor: &Actor<'_>, ctx: &Context<'_>) -> Option<Target> {
    let mut best = None;
    let mut shortest = u32::MAX;
    for target in ctx
        .class3
        .iter()
        .filter_map(|slot| ctx.target(*slot))
        .filter(|target| target.source_candidate())
    {
        let d = xy_distance_squared(actor.row.position, target.position);
        if d < shortest {
            shortest = d;
            best = Some(target);
        }
    }
    best
}

fn move_forward(actor: &mut Actor<'_>, ctx: &mut Context<'_>) {
    move_source_actor(actor.row, ctx.terrain, ctx.population.as_deref_mut());
}

pub(super) fn move_source_actor(
    row: &mut EntitySnapshot,
    terrain: &SessionTerrain,
    population: Option<&mut u32>,
) {
    if row.control_flags & 0x800 != 0 {
        row.set_control_flags(row.control_flags & !0x800);
        return;
    }
    let motion = crate::ground_creature_motion::step_ground_creature_motion(
        row.position,
        row.yaw,
        row.source_roll(),
        row.actual_speed,
        terrain,
        crate::ground_creature_motion::GroundCreatureMotionConfig {
            half_width: row.array_pitch(),
            half_height: row.array_fov(),
            ground_clearance: RULES.ground_clearance,
            descent: RULES.descent,
            turn_limit: RULES.turn_limit,
            terrain_mask: RULES.terrain_mask,
            maximum_tile_slope: RULES.maximum_slope,
        },
    );
    row.set_position(motion.position);
    row.set_yaw(motion.yaw);
    let flags = if motion.movement_blocked {
        row.control_flags | 0x40000
    } else {
        row.control_flags & !0x40000
    };
    row.set_control_flags(flags);
    if motion.killed {
        if RULES.behavior_flags & 1 != 0
            || terrain.material_mask(row.position) == crate::WATER_AMBIENCE_MASK
        {
            row.set_life(-1);
        }
        if row.life == -1
            && let Some(population) = population
            && *population != 0
        {
            *population -= 1;
        }
    }
}

fn neighbor_avoidance(actor: &mut Actor<'_>, ctx: &Context<'_>) -> Result<(), String> {
    for read in ctx.peers {
        let Some(other) = read.as_ref().map_err(Clone::clone)? else {
            continue;
        };
        if other.source_id == actor.row.source_id {
            continue;
        }
        let dx = i32::from(other.position.x) - i32::from(actor.row.position.x);
        let dy = i32::from(other.position.y) - i32::from(actor.row.position.y);
        if dx.abs() < i32::from(actor.row.array_pitch())
            && dy.abs() < i32::from(actor.row.array_pitch())
        {
            actor
                .row
                .set_source_roll(heading_for_positions(other.position, actor.row.position) as i16);
            break;
        }
    }
    Ok(())
}

fn approach(actor: &mut Actor<'_>, ctx: &mut Context<'_>, target: Position) -> bool {
    // remc2's EuclideanDistXYZ_58490 uses XY only; Z has a separate
    // target+640 test. Both tests use the position before their adjustment.
    let mut ready = true;
    if xy_distance(actor.row.position, target) > 128 {
        ready = false;
        actor
            .row
            .set_source_roll(heading_for_positions(actor.row.position, target) as i16);
        move_forward(actor, ctx);
    }
    let delta =
        i32::from(target.z) + i32::from(RULES.lift_spacing) - i32::from(actor.row.position.z);
    if delta.unsigned_abs() > u32::from(RULES.lift_tolerance as u16) {
        ready = false;
        let mut position = actor.row.position;
        position.z = position.z.wrapping_add(if delta < 0 {
            -RULES.lift_step
        } else {
            RULES.lift_step
        });
        actor.row.set_position(position);
    }
    ready
}

fn action_idle(actor: &mut Actor<'_>, ctx: &mut Context<'_>) -> Result<(), String> {
    move_forward(actor, ctx);
    neighbor_avoidance(actor, ctx)?;
    let height_delta = i32::from(actor.row.position.z) - i32::from(actor.target_height());
    if height_delta.unsigned_abs() >= 256 {
        let mut position = actor.row.position;
        position.z = position.z.wrapping_add(if height_delta <= 0 { 32 } else { -32 });
        actor.row.set_position(position);
    }
    match actor.mode() {
        0 => {
            actor.row.set_actual_speed(RULES.maximum_speed);
            let timer = actor.timer();
            actor.set_timer(timer.wrapping_sub(1));
            if timer == 0 {
                actor.set_mode(1);
                actor.set_timer(0);
            }
        }
        1 => {
            if let Some(sphere) = nearest_sphere(actor, ctx)? {
                actor.set_target_height(RULES.maximum_lift);
                actor.set_target(sphere.slot);
                actor.set_mode(2);
                actor.set_timer(0);
            } else {
                let ground = ctx.terrain.ground_height(actor.row.position);
                actor.set_target_height(ground.wrapping_add(7 * 256));
                actor.set_timer(80);
                actor.set_mode(0);
            }
        }
        2 => {
            let Some(sphere) = ctx
                .sphere(actor.target())
                .filter(|sphere| living_sphere(*sphere))
            else {
                actor.set_mode(1);
                actor.set_timer(0);
                return Ok(());
            };
            if actor.row.phase & 3 == 0 {
                actor.row.set_source_roll(heading_for_positions(actor.row.position, sphere.position) as i16);
                if xy_distance(actor.row.position, sphere.position) < RULES.approach_range as u32 {
                    actor.set_action(185);
                    actor.set_mode(0);
                    actor.set_timer(500);
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn crowded_above(actor: &mut Actor<'_>, ctx: &Context<'_>) -> Result<bool, String> {
    for read in ctx.peers {
        let Some(other) = read.as_ref().map_err(Clone::clone)? else {
            continue;
        };
        if other.source_id == actor.row.source_id {
            continue;
        }
        let dx = i32::from(actor.row.position.x) - i32::from(other.position.x);
        let dy = i32::from(actor.row.position.y) - i32::from(other.position.y);
        let dz = i32::from(actor.row.position.z) - i32::from(other.position.z);
        let spacing = i32::from(actor.row.array_pitch()).wrapping_mul(2);
        let height = i32::from(actor.row.array_fov()).wrapping_mul(2);
        if dx.abs() < spacing
            && dy.abs() < spacing
            && dz.abs() < height
            && actor.row.position.z >= other.position.z
        {
            let mut position = actor.row.position;
            position.z = position.z.wrapping_add(16);
            actor.row.set_position(position);
            return Ok(true);
        }
    }
    Ok(false)
}

fn action_approach_sphere(actor: &mut Actor<'_>, ctx: &mut Context<'_>) -> Result<(), String> {
    if actor.mode() != 0 {
        if actor.mode() == 1 {
            if actor.row.position.z >= RULES.maximum_lift {
                actor.set_action(184);
                actor.set_mode(0);
                actor.set_timer(80);
            } else {
                let mut position = actor.row.position;
                position.z = position.z.wrapping_add(32);
                actor.row.set_position(position);
            }
        }
        return Ok(());
    }
    actor.row.set_actual_speed(RULES.minimum_speed);
    let timer = actor.timer().wrapping_sub(1);
    actor.set_timer(timer);
    let Some(sphere) = ctx
        .sphere(actor.target())
        .filter(|sphere| living_sphere(*sphere))
    else {
        actor.set_mode(1);
        actor.set_timer(0);
        return Ok(());
    };
    if timer == 0 || crowded_above(actor, ctx)? {
        actor.set_mode(1);
        actor.set_timer(0);
        return Ok(());
    }
    if approach(actor, ctx, sphere.position) {
        // The source enters action187's sphere capture callback.
        actor.set_action(187);
        actor.set_mode(0);
        actor.set_timer(0);
    }
    Ok(())
}

fn detach_sphere(actor: &Actor<'_>, ctx: &mut Context<'_>) {
    let target_slot = actor.target();
    let Some(sphere) = ctx
        .spheres
        .iter_mut()
        .find(|sphere| sphere.source_entity_index == target_slot && living_sphere(**sphere))
    else {
        return;
    };
    if sphere.control_flags & MANA_SPHERE_CLAIMED != 0
        && sphere.assigned_defender_slot == actor.row.index
    {
        sphere.control_flags &= !MANA_SPHERE_CLAIMED;
        sphere.assigned_carrier_id = 0;
        sphere.assigned_defender_slot = 0;
        if let Some(row) = ctx.entities.get_mut(usize::from(target_slot)) {
            row.set_control_flags(row.control_flags & !MANA_SPHERE_CLAIMED);
            row.record[154..156].copy_from_slice(&0_u16.to_le_bytes());
        }
    }
}

fn action_absorb_sphere(actor: &mut Actor<'_>, ctx: &mut Context<'_>) -> Result<(), String> {
    let mode = actor.mode();
    let mut abandon = false;
    let mut climb = false;
    if mode == 0 {
        if ctx.sphere(actor.target()).is_none_or(|sphere| {
            !living_sphere(sphere) || sphere.control_flags & MANA_SPHERE_CLAIMED != 0
        }) {
            // Source sets v9 but still processes a valid, already-claimed
            // sphere below before taking the mode-3 tail.
            abandon = true;
        } else {
            actor.set_target_height(18);
            actor.set_action(187);
            actor.set_mode(1);
            actor.set_timer(64);
        }
    } else if mode == 3 {
        actor.set_target(0);
        if let Some(sphere) = nearest_sphere(actor, ctx)? {
            actor.set_target(sphere.slot);
            if xy_distance(actor.row.position, sphere.position) <= RULES.vulnerable_target_range {
                actor.set_action(185);
                actor.set_mode(0);
                actor.set_timer(500);
                return Ok(());
            }
        }
        actor.set_action(185);
        actor.set_mode(1);
        actor.set_timer(0);
        return Ok(());
    } else if mode != 1 {
        return Ok(());
    }
    if actor.target() == 0 {
        climb = true;
    } else if let Some(snapshot) = ctx
        .sphere(actor.target())
        .filter(|sphere| living_sphere(*sphere))
    {
        let timer = actor.timer().wrapping_sub(1);
        actor.set_timer(timer);
        if timer == 0 {
            abandon = true;
        } else if let Some(sphere) = ctx.spheres.iter_mut().find(|sphere| {
            sphere.active && sphere.source_entity_index == snapshot.source_entity_index
        }) {
            let target_slot = sphere.source_entity_index;
            sphere.control_flags |= MANA_SPHERE_CLAIMED;
            // Source word96 is the holder slot; assigned_carrier_id is for
            // actual class-3/model-3 carriers.
            sphere.assigned_carrier_id = 0;
            sphere.assigned_defender_slot = actor.row.index;
            if let Some(row) = ctx.entities.get_mut(usize::from(target_slot)) {
                row.set_control_flags(row.control_flags | MANA_SPHERE_CLAIMED);
                row.record[154..156].copy_from_slice(&actor.row.index.to_le_bytes());
            }
            actor.set_target_height(actor.target_height().wrapping_add(10));
            let overlaps = crate::native_world::level2_model76_sub106c0_matches(
                actor.row.position,
                actor.row.array_yaw(),
                actor.row.array_pitch(),
                actor.row.array_roll(),
                actor.row.array_fov(),
                sphere.position,
                sphere.collision_yaw,
                sphere.collision_pitch,
                sphere.collision_roll,
                sphere.collision_fov,
            );
            if overlaps || sphere.position.z > actor.row.position.z {
                actor.row.set_mana(actor.row.mana.wrapping_add(sphere.mana));
                sphere.control_flags |= ENTITY_DRAW_DISABLED_BIT;
                sphere.pending_delete = true;
                abandon = true;
                if let Some(row) = ctx.entities.get_mut(usize::from(target_slot)) {
                    row.disable_drawing();
                }
            }
        }
    } else {
        abandon = true;
    }
    if abandon {
        actor.set_action(187);
        actor.set_mode(3);
        actor.set_timer(0);
    }
    if climb {
        actor.set_action(185);
        actor.set_mode(1);
        actor.set_timer(0);
    }
    Ok(())
}

fn action_attack(actor: &mut Actor<'_>, ctx: &mut Context<'_>, effects: &mut Effects) -> Result<(), String> {
    actor.row.set_actual_speed(RULES.minimum_speed);
    // sub_27E00 moves and turns before resolving its current target.
    move_forward(actor, ctx);
    let target = (actor.target() != 0).then(|| actor.target());
    action_attack_after_move(actor, ctx, effects, target)
}

fn action_attack_after_move(
    actor: &mut Actor<'_>,
    ctx: &mut Context<'_>,
    effects: &mut Effects,
    target_slot: Option<u16>,
) -> Result<(), String> {
    if let Some(error) = ctx.target_error {
        return Err(error.to_owned());
    }
    let Some(target_slot) = target_slot else {
        effects.attack_abandon = true;
        return Ok(());
    };
    let Some(mut target) = ctx.target(target_slot) else {
        effects.attack_abandon = true;
        return Ok(());
    };
    if target_slot == actor.row.index {
        target.position = actor.row.position;
        target.life = actor.row.life;
        target.control_flags = actor.row.control_flags;
    }
    if !target.living() {
        // Ordinary victim selection in sub282D0 is class3; class10/model39
        // relations enter the separate action187 sphere-capture path.
        effects.attack_abandon = true;
        return Ok(());
    }
    let (target_slot, target_position) = (target.slot, target.position);
    if actor.row.phase & 3 == 0 {
        actor
            .row
            .set_source_roll(heading_for_positions(actor.row.position, target_position) as i16);
        neighbor_avoidance(actor, ctx)?;
    }
    if actor.row.phase & RULES.scan_period == 0 {
        if distance(actor.row.position, target_position) >= RULES.target_range {
            effects.attack_abandon = true;
            return Ok(());
        }
        let launch = Position {
            z: actor.row.position.z.wrapping_add(actor.row.array_fov()),
            ..actor.row.position
        };
        effects.lightning = Some(LightningRequest {
            source_slot: actor.row.index,
            target_slot,
            position: launch,
            damage: RULES.lightning_damage,
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CombatTail {
    Continue,
    Retaliate(u16),
    Dead,
}

fn consume_combat_tail(actor: &mut Actor<'_>, sentinel_class3: bool) -> CombatTail {
    let mut damage = i32::from_le_bytes(
        actor.row.record[98..102]
            .try_into()
            .expect("fixed source damage mailbox"),
    );
    let mut pending = actor.word(102);
    let mut attacker = actor.word(42);
    let mut life = actor.row.life;
    let consumed = if actor.row.record[61] != 0 {
        crate::combat::actors::consume_damage(
            &mut life,
            &mut damage,
            &mut pending,
            &mut attacker,
        ).is_some()
    } else { false };
    actor.row.set_life(life);
    actor.row.record[98..102].copy_from_slice(&damage.to_le_bytes());
    actor.put(102, pending);
    actor.put(42, attacker);
    // Pinned sub_28110 initializes its attacker-class lookup to slot 0.
    // The ordinary sentinel is class0, but a configured current class3
    // sentinel selects retaliation before the independent nearest scan.
    if life < 0 {
        actor.put(40, attacker);
        actor.set_action(188);
        actor.set_mode(0);
        actor.set_timer(0);
        return CombatTail::Dead;
    }
    if consumed && sentinel_class3 { CombatTail::Retaliate(attacker) } else { CombatTail::Continue }
}

fn finish_combat_tail(actor: &mut Actor<'_>, ctx: &mut Context<'_>, tail: CombatTail) {
    if let CombatTail::Retaliate(attacker) = tail {
        detach_sphere(actor, ctx);
        actor.set_target(attacker);
        actor.set_action(186);
        actor.set_mode(0);
        actor.set_timer(0);
        return;
    }
    if tail == CombatTail::Dead { return; }
    if actor.row.phase & 0x1f == 0 {
        if let Some(target) = nearest_victim(actor, ctx)
            && distance(actor.row.position, target.position) < RULES.target_range
        {
            detach_sphere(actor, ctx);
            actor.set_target(target.slot);
            actor.set_action(186);
            actor.set_mode(0);
            actor.set_timer(0);
        }
    }
}

fn finish_attack(actor: &mut Actor<'_>, effects: &mut Effects) {
    if effects.attack_abandon {
        actor.set_target(0);
        actor.set_action(187);
        actor.set_mode(3);
        actor.set_timer(0);
        effects.attack_abandon = false;
    }
}

fn step(actor: &mut Actor<'_>, ctx: &mut Context<'_>) -> Result<Effects, String> {
    let mut effects = Effects::default();
    let entered_action = actor.action();
    match actor.action() {
        184 => action_idle(actor, ctx)?,
        185 => action_approach_sphere(actor, ctx)?,
        186 => action_attack(actor, ctx, &mut effects)?,
        187 => action_absorb_sphere(actor, ctx)?,
        188 => actor.set_action(189),
        189 => effects.kill = actor.row.phase & 7 == 0,
        191 => return Err("model23 action191 requires the source-arena stage owner".to_owned()),
        action => return Err(format!("model23 action{action} has no source callback")),
    }
    if matches!(entered_action, 184..=187) {
        effects.combat_tail_pending = true;
    } else if actor.action() != 189 {
        let tail = consume_combat_tail(actor, ctx.entities.first().is_some_and(|row| row.class == 3));
        finish_combat_tail(actor, ctx, tail);
    }
    Ok(effects)
}
