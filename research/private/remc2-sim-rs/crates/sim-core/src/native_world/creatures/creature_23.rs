//! Model23 flying mana predator behavior (MC2 actions184..191).
//!
//! The source arena row remains authoritative. Action186 commits Lightning
//! attacks, action187 captures ManaSpheres, and the leaf adapter owns their
//! product and reclaim effects.

use super::archer::ArcherStageRuntime as StageRuntime;
pub(crate) use crate::level_definition::CREATURE23_CLASS_PROPERTIES as RULES;
use crate::native_world::{
    ENTITY_DRAW_DISABLED_BIT, EntitySnapshot, MapType, SessionTerrain, level2_next_rng16,
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
        self.living()
            && self.class == 3
            && (matches!(self.model, 0 | 1) || (self.model == 3 && self.mana > 0))
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
    /// Frozen mana-chain addresses resolved to their current model39 owners.
    pub mana_candidates: &'a [ManaCandidate],
    pub class3: &'a [u16],
    pub families: &'a [Vec<u16>],
    pub entities: &'a mut [EntitySnapshot],
    pub spheres: &'a mut [crate::level1_mission::ManaSphere],
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
    pub source_id: u16,
    pub target_slot: u16,
    pub target_class: u8,
    pub target_model: u8,
    pub position: Position,
    pub target_position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub damage: i32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Effects {
    /// Source sub_1D260: allocate class9/model9 first, then let its actual
    /// 10/23 consumer apply damage to this target.
    pub lightning: Option<LightningRequest>,
    /// Player feedback published only after the source Ray exists and its
    /// target receives the `sub_5EF70` write.
    pub target_music: Option<u16>,
    pub sound: Option<u8>,
    pub kill_credit: Option<u16>,
    pub kill: bool,
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
) -> Option<ManaCandidate> {
    let mut best = None;
    let mut shortest = RULES.mana_sphere_range;
    // sub_28000 checks model only on the frozen list's current occupants;
    // sub_28420 validates life, hide and class on the later target visit.
    for sphere in ctx.mana_candidates.iter().copied() {
        let d = xy_distance_squared(actor.row.position, sphere.position);
        if d < shortest {
            shortest = d;
            best = Some(sphere);
        }
    }
    best
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

fn move_forward(actor: &mut Actor<'_>, ctx: &Context<'_>) {
    let mut position = actor.row.position;
    crate::move_entity(&mut position, actor.row.yaw, 0, actor.row.actual_speed);
    let ground = ctx.terrain.ground_height(position);
    if position.z < ground {
        position.z = ground;
    }
    if ctx.terrain.map_type() == MapType::Cave
        && let Some(ceiling) = ctx.terrain.ceiling_height(position)
        && position.z > ceiling.wrapping_sub(actor.row.array_fov())
    {
        position.z = ceiling.wrapping_sub(actor.row.array_fov());
    }
    actor.row.set_position(position);
    // sub_1B8C0 turns toward roll after committing movement. Descriptor91
    // supplies the source 0x100 angular step.
    let yaw = actor.row.yaw;
    let roll = actor.row.source_roll();
    actor.row.set_yaw(crate::wrap_angle(
        i32::from(yaw) + i32::from(crate::turn_toward(yaw, roll, 0x100)),
    ));
}

fn neighbor_avoidance(actor: &mut Actor<'_>, ctx: &Context<'_>) {
    for slot in ctx
        .families
        .get(usize::from(SOURCE_MODEL))
        .into_iter()
        .flatten()
        .copied()
    {
        if slot == actor.row.index {
            continue;
        }
        let Some(other) = ctx
            .entities
            .get(usize::from(slot))
            .filter(|other| other.active && other.class == 5 && other.model == SOURCE_MODEL)
        else {
            continue;
        };
        let dx = i32::from(other.position.x.wrapping_sub(actor.row.position.x) as i16);
        let dy = i32::from(other.position.y.wrapping_sub(actor.row.position.y) as i16);
        if dx.unsigned_abs() < RULES.collision_spacing as u32
            && dy.unsigned_abs() < RULES.collision_spacing as u32
        {
            actor
                .row
                .set_source_roll(heading_for_positions(other.position, actor.row.position) as i16);
            break;
        }
    }
}

fn approach(actor: &mut Actor<'_>, ctx: &Context<'_>, target: Position) -> bool {
    // remc2's EuclideanDistXYZ_58490 uses XY only; Z has a separate
    // target+640 test. Both tests use the position before their adjustment.
    let mut ready = true;
    if xy_distance(actor.row.position, target) > 128 {
        ready = false;
        actor.row.set_source_roll(heading_for_positions(actor.row.position, target) as i16);
        move_forward(actor, ctx);
    }
    let delta = i32::from(target.z) + i32::from(RULES.lift_spacing)
        - i32::from(actor.row.position.z);
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

fn action_idle(actor: &mut Actor<'_>, ctx: &mut Context<'_>) {
    move_forward(actor, ctx);
    neighbor_avoidance(actor, ctx);
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
            if let Some(sphere) = nearest_sphere(actor, ctx) {
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
                return;
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
}

fn crowded_above(actor: &mut Actor<'_>, ctx: &Context<'_>) -> bool {
    let Some(family) = ctx.families.get(usize::from(SOURCE_MODEL)) else {
        return false;
    };
    for slot in family
        .iter()
        .copied()
        .filter(|slot| *slot != actor.row.index)
    {
        let Some(other) = ctx
            .entities
            .get(usize::from(slot))
            .filter(|other| other.active && other.class == 5 && other.model == SOURCE_MODEL)
        else {
            continue;
        };
        let dx = i32::from(actor.row.position.x.wrapping_sub(other.position.x) as i16);
        let dy = i32::from(actor.row.position.y.wrapping_sub(other.position.y) as i16);
        let dz = i32::from(actor.row.position.z.wrapping_sub(other.position.z) as i16);
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
            return true;
        }
    }
    false
}

fn action_approach_sphere(actor: &mut Actor<'_>, ctx: &mut Context<'_>) {
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
        return;
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
        return;
    };
    if timer == 0 || crowded_above(actor, ctx) {
        actor.set_mode(1);
        actor.set_timer(0);
        return;
    }
    if approach(actor, ctx, sphere.position) {
        // The source enters action187's sphere capture callback.
        actor.set_action(187);
        actor.set_mode(0);
        actor.set_timer(0);
    }
}

fn detach_sphere(actor: &Actor<'_>, ctx: &mut Context<'_>) {
    let target_slot = actor.target();
    let Some(sphere) = ctx
        .spheres
        .iter_mut()
        .find(|sphere| sphere.active && sphere.source_entity_index == target_slot)
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

fn action_absorb_sphere(actor: &mut Actor<'_>, ctx: &mut Context<'_>) {
    let mode = actor.mode();
    let sphere_snapshot = ctx.sphere(actor.target());
    if mode == 0 {
        if sphere_snapshot.is_none_or(|sphere| {
            !living_sphere(sphere) || sphere.control_flags & MANA_SPHERE_CLAIMED != 0
        }) {
            if actor.target() == 0 {
                actor.set_action(185);
                actor.set_mode(1);
            } else {
                actor.set_action(187);
                actor.set_mode(3);
            }
            return;
        } else {
            actor.set_target_height(18);
            actor.set_action(187);
            actor.set_mode(1);
            actor.set_timer(64);
        }
    } else if mode == 3 {
        actor.set_target(0);
        if let Some(sphere) = nearest_sphere(actor, ctx)
            && xy_distance(actor.row.position, sphere.position) <= RULES.vulnerable_target_range
        {
            actor.set_target(sphere.slot);
            actor.set_action(185);
            actor.set_mode(0);
            actor.set_timer(500);
        } else {
            actor.set_action(185);
            actor.set_mode(1);
        }
        return;
    }

    if actor.target() == 0 {
        actor.set_action(185);
        actor.set_mode(1);
        actor.set_timer(0);
        return;
    }
    let timer = actor.timer().wrapping_sub(1);
    actor.set_timer(timer);
    let Some(sphere_snapshot) = ctx
        .sphere(actor.target())
        .filter(|sphere| living_sphere(*sphere))
    else {
        actor.set_mode(3);
        return;
    };
    if timer == 0 {
        actor.set_mode(3);
        return;
    }
    let target_slot = sphere_snapshot.source_entity_index;
    let Some(sphere) = ctx
        .spheres
        .iter_mut()
        .find(|sphere| sphere.active && sphere.source_entity_index == target_slot)
    else {
        actor.set_mode(3);
        return;
    };
    sphere.control_flags |= MANA_SPHERE_CLAIMED;
    // Source word96 is the holder slot. Keep it in the typed slot relation;
    // assigned_carrier_id is reserved for actual class-3/model-3 carriers.
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
        actor.set_mode(3);
        if let Some(row) = ctx.entities.get_mut(usize::from(target_slot)) {
            row.disable_drawing();
        }
    }
}

fn action_attack(actor: &mut Actor<'_>, ctx: &mut Context<'_>, effects: &mut Effects) {
    actor.row.set_actual_speed(RULES.minimum_speed);
    let target_slot = actor.target();
    let Some(target) = ctx.target(target_slot).filter(|target| target.living()) else {
        // Ordinary victim selection in sub282D0 is class3; class10/model39
        // relations enter the separate action187 sphere-capture path.
        actor.set_target(0);
        actor.set_mode(3);
        actor.set_action(187);
        return;
    };
    let (target_slot, target_class, target_model, target_position) =
        (target.slot, target.class, target.model, target.position);
    if actor.row.phase & 3 == 0 {
        actor
            .row
            .set_yaw(heading_for_positions(actor.row.position, target_position) as i16);
        actor.row.set_source_roll(actor.row.yaw);
        neighbor_avoidance(actor, ctx);
        if actor.row.phase & RULES.scan_period == 0
            && distance(actor.row.position, target_position) < RULES.target_range
        {
            let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(
                Position {
                    z: actor.row.position.z.wrapping_add(actor.row.array_fov()),
                    ..actor.row.position
                },
                target_position,
            );
            effects.lightning = Some(LightningRequest {
                source_slot: actor.row.index,
                source_id: actor.row.source_id,
                target_slot,
                target_class,
                target_model,
                position: Position {
                    z: actor.row.position.z.wrapping_add(actor.row.array_fov()),
                    ..actor.row.position
                },
                target_position,
                yaw,
                pitch,
                damage: RULES.lightning_damage,
            });
        }
    }
    move_forward(actor, ctx);
}

fn combat_tail(actor: &mut Actor<'_>, ctx: &mut Context<'_>, effects: &mut Effects) {
    let mut damage = i32::from_le_bytes(
        actor.row.record[98..102]
            .try_into()
            .expect("fixed source damage mailbox"),
    );
    let mut pending = actor.word(102);
    let mut attacker = actor.word(42);
    let mut life = actor.row.life;
    if actor.row.record[61] != 0 {
        let _ = crate::combat::actors::consume_damage(
            &mut life, &mut damage, &mut pending, &mut attacker,
        );
    }
    actor.row.set_life(life);
    actor.row.record[98..102].copy_from_slice(&damage.to_le_bytes());
    actor.put(102, pending);
    actor.put(42, attacker);
    // Pinned sub_28110 initializes its attacker-class lookup to slot 0.
    // A normal player hit therefore does not enter the retaliation branch;
    // the independent phase-gated nearest-victim scan below still runs.
    if life < 0 {
        actor.put(40, attacker);
        actor.set_action(188);
        actor.set_mode(0);
        actor.set_timer(0);
        return;
    }
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

fn step(actor: &mut Actor<'_>, ctx: &mut Context<'_>) -> Result<Effects, String> {
    let mut effects = Effects::default();
    match actor.action() {
        184 => action_idle(actor, ctx),
        185 => action_approach_sphere(actor, ctx),
        186 => {
            action_attack(actor, ctx, &mut effects);
            effects.sound = Some(59);
        }
        187 => {
            action_absorb_sphere(actor, ctx);
            effects.sound = Some(59);
        }
        188 => actor.set_action(189),
        189 => effects.kill = actor.row.phase & 7 == 0,
        191 => stage::step(actor, ctx),
        action => return Err(format!("model23 action{action} has no source callback")),
    }
    if actor.action() != 189 {
        combat_tail(actor, ctx, &mut effects);
    }
    Ok(effects)
}
