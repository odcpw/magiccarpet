//! Worm action31: source `sub_1FA50 -> sub_1D5D0(24)`.
//!
//! The adapter lends this callback its current actor packet, commits it before
//! terminal expiry allocation, and retains the common source epilogue/reclaim.

use crate::campaign::source_arena::SourceArena;
use crate::combat::area::Shape;
use crate::level_definition::WORM_PROPERTIES;
use crate::native_world::{
    EntitySnapshot, RuntimeRelation, RuntimeSnapshot, RuntimeStageVariable, SessionTerrain,
    level2_next_rng16, source_target,
};
use crate::{FlightWorld, Position, angle_distance, heading_for_positions};

// Source Level.cpp descriptor74. It never selects the unsupported base+6.
const RANGE: u32 = WORM_PROPERTIES.target_range;
const CADENCE: u8 = WORM_PROPERTIES.scan_period;
const HALF_ANGLE: i16 = WORM_PROPERTIES.target_half_angle;
const FLAGS: u8 = WORM_PROPERTIES.behavior_flags;

#[derive(Debug, Default)]
pub(crate) struct Effects {
    pub population_decrements: u32,
    /// Terminal source1E580 births: true adds10/73 before10/0; false only10/0.
    /// Commit the actor first, allocate from each CURRENT pose, then hide the
    /// actual current occupant. Never write the old actor over a replacement.
    pub expiry: Option<bool>,
    pub spell_awards: Vec<(u16, u8)>,
}

struct Context<'a> {
    arena: &'a mut SourceArena,
    runtime: &'a RuntimeSnapshot,
    terrain: &'a SessionTerrain,
    models: &'a [Vec<u16>; 29],
    class3: &'a [u16],
    targets: source_target::Context<'a>,
}

fn word(actor: &EntitySnapshot, at: usize) -> u16 {
    u16::from_le_bytes([actor.record[at], actor.record[at + 1]])
}

fn put(actor: &mut EntitySnapshot, at: usize, value: u16) {
    actor.record[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn required<T>(slot: u16, value: Option<T>, field: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("Worm stage current target{slot} lacks {field}"))
}

fn packet_view(actor: &EntitySnapshot) -> source_target::View {
    source_target::View {
        slot: actor.index,
        source_id: actor.source_id,
        active: actor.active,
        class: actor.class,
        model: actor.model,
        position: actor.position,
        life: actor.life,
        control_flags: actor.control_flags,
        yaw: Some(actor.yaw),
        roll: Some(actor.source_roll()),
        relations: source_target::Relations {
            target: Some(word(actor, 154)),
            attacker: Some(word(actor, 42)),
            parent: Some(word(actor, 44)),
            next: Some(word(actor, 56)),
            stage_mode: Some(actor.record[77]),
            stage_index: Some(actor.record[76]),
            stage_relation: Some(word(actor, 78)),
        },
        collision: Some(Shape {
            half_width: actor.array_pitch(),
            half_depth: actor.array_roll(),
            vertical_offset: actor.array_yaw(),
            half_height: actor.array_fov(),
        }),
    }
}

fn current(
    actor: &EntitySnapshot,
    ctx: &Context<'_>,
    slot: u16,
) -> Result<Option<source_target::View>, String> {
    if slot == actor.index {
        return Ok(Some(packet_view(actor)));
    }
    source_target::read(ctx.arena, ctx.targets, slot)
}

fn sentinel(ctx: &Context<'_>) -> source_target::View {
    packet_view(&ctx.arena.entities[0])
}

fn route(actor: &EntitySnapshot, ctx: &Context<'_>) -> Result<RuntimeStageVariable, String> {
    ctx.runtime
        .stage_variables
        .get(usize::from(actor.record[76]))
        .copied()
        .ok_or_else(|| {
            format!(
                "Worm action31 StageVar{} is out of bounds",
                actor.record[76]
            )
        })
}

fn point(stage: RuntimeStageVariable) -> Result<Position, String> {
    let value = match stage.relation {
        RuntimeRelation::AxisScalar(value) | RuntimeRelation::Scalar(value) => value,
        _ => return Err("Worm point StageVar has no source coordinate".into()),
    };
    Ok(Position {
        x: value as u16,
        y: (value >> 16) as u16,
        z: 0,
    })
}

fn draw(actor: &mut EntitySnapshot) -> u16 {
    let rng = level2_next_rng16(actor.entity_rng);
    actor.set_entity_rng(rng);
    rng
}

fn distance_xy(a: Position, b: Position) -> u32 {
    let dx = i32::from(a.x.wrapping_sub(b.x) as i16);
    let dy = i32::from(a.y.wrapping_sub(b.y) as i16);
    dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32
}

fn distance(a: Position, b: Position) -> u32 {
    let dz = i32::from(a.z.wrapping_sub(b.z));
    crate::legacy_sqrt(distance_xy(a, b).wrapping_add(dz.wrapping_mul(dz) as u32))
}

fn attack(actor: &mut EntitySnapshot) -> Result<(), String> {
    if FLAGS & 8 != 0 {
        return Err("Worm source flee selects unsupported action30".into());
    }
    actor.set_source_action(26);
    Ok(())
}

/// Each generic StageVar helper has this prelude. A linked lower life is read
/// in source chain order, and its attacker is read only when that life wins.
fn damage(actor: &mut EntitySnapshot, ctx: &Context<'_>) -> Result<u8, String> {
    let owner = word(actor, 102);
    let mut hit = owner != 0;
    if hit {
        let payload = i32::from_le_bytes(actor.record[98..102].try_into().unwrap());
        #[cfg(test)]
        let consumption = ctx.arena.handle(actor.index).map(|target| {
            (
                target,
                actor.life,
                crate::native_world::area_damage::current_actor_mailbox_writer(
                    target,
                    (payload, owner),
                ),
            )
        });
        actor.set_life(actor.life.wrapping_sub(payload));
        put(actor, 42, owner);
        put(actor, 102, 0);
        #[cfg(test)]
        if let Some((target, life_before, writer)) = consumption {
            crate::native_world::area_damage::trace_player_damage_consumption(
                crate::native_world::area_damage::PlayerDamageConsumptionReceipt {
                    target,
                    mailbox: (payload, owner),
                    writer,
                    life_before,
                    life_after: actor.life,
                    event_cursor: ctx.arena.economy.events().len(),
                },
            );
            let _ = crate::native_world::area_damage::take_player_mailbox_writer(
                target,
                (payload, owner),
            );
        }
    } else {
        put(actor, 42, 0);
    }
    let mut child = word(actor, 56);
    let mut remaining = ctx.arena.entities.len();
    while child != 0 {
        if remaining == 0 {
            return Err("Worm stage has a cyclic linked damage chain".into());
        }
        remaining -= 1;
        let Some(view) = current(actor, ctx, child)? else {
            break;
        };
        if view.life < actor.life {
            #[cfg(test)]
            let life_before = actor.life;
            actor.set_life(view.life);
            put(
                actor,
                42,
                required(child, view.relations.attacker, "linked attacker")?,
            );
            #[cfg(test)]
            super::mc2::record_inherited_damage(
                ctx.arena,
                actor.index,
                child,
                life_before,
                view.life,
                word(actor, 42),
            );
            hit = true;
            break;
        }
        child = required(child, view.relations.next, "linked next")?;
    }
    if actor.life < 0 {
        let attacker = word(actor, 42);
        put(actor, 40, attacker);
        Ok(2)
    } else {
        Ok(u8::from(hit))
    }
}

fn movement(actor: &mut EntitySnapshot, ctx: &Context<'_>, effects: &mut Effects) {
    effects.population_decrements +=
        u32::from(super::imported::move_stage_actor(actor, ctx.terrain));
}

fn relax(actor: &mut EntitySnapshot, ctx: &Context<'_>) {
    super::imported::settle_stage_actor(actor, ctx.terrain);
}

fn different_family(actor: &EntitySnapshot, ctx: &Context<'_>, slot: u16) -> Result<bool, String> {
    let target = if slot == 0 {
        Some(sentinel(ctx))
    } else {
        current(actor, ctx, slot)?
    };
    Ok(target.is_none_or(|target| (target.class, target.model) != (actor.class, actor.model)))
}

fn hit_reaction(actor: &mut EntitySnapshot, ctx: &Context<'_>) -> Result<(), String> {
    let attacker = word(actor, 42);
    if different_family(actor, ctx, attacker)? {
        put(actor, 154, attacker);
        actor.record[77] = 10;
        attack(actor)?;
    }
    relax(actor, ctx);
    Ok(())
}

fn wander_roll(actor: &mut EntitySnapshot) {
    let rng = draw(actor);
    actor.set_source_roll(actor.source_roll().wrapping_add((rng % 113 + 142) as i16) & 2047);
}

fn aim_roll(actor: &mut EntitySnapshot, destination: Position) {
    actor.set_source_roll(heading_for_positions(actor.position, destination) as i16);
    if actor.phase & 63 == 0 {
        let sign = 2 * ((draw(actor) % 157) / 79) as i16 - 1;
        let amount = (draw(actor) & 255) as i16 + 85;
        actor.set_source_roll(actor.source_roll().wrapping_add(sign.wrapping_mul(amount)) & 2047);
    }
}

fn avoid(actor: &mut EntitySnapshot, ctx: &Context<'_>, signed: bool) -> Result<(), String> {
    for &slot in &ctx.models[3] {
        let Some(peer) = current(actor, ctx, slot)? else {
            continue;
        };
        let (dx, dy) = if signed {
            (
                i32::from(actor.position.x as i16) - i32::from(peer.position.x as i16),
                i32::from(actor.position.y as i16) - i32::from(peer.position.y as i16),
            )
        } else {
            (
                i32::from(actor.position.x) - i32::from(peer.position.x),
                i32::from(actor.position.y) - i32::from(peer.position.y),
            )
        };
        if peer.source_id != actor.source_id
            && dx.abs() < i32::from(actor.array_pitch())
            && dy.abs() < i32::from(actor.array_pitch())
        {
            actor.set_source_roll(heading_for_positions(peer.position, actor.position) as i16);
            break;
        }
    }
    Ok(())
}

fn wander(
    actor: &mut EntitySnapshot,
    ctx: &Context<'_>,
    effects: &mut Effects,
) -> Result<(), String> {
    match damage(actor, ctx)? {
        0 if FLAGS & 2 == 0 => {
            movement(actor, ctx, effects);
            if actor.phase & 15 == 0 && actor.control_flags & 0x40000 == 0 {
                wander_roll(actor);
            }
        }
        1 => hit_reaction(actor, ctx)?,
        2 => actor.set_source_action(28),
        _ => {}
    }
    Ok(())
}

fn point_step(
    actor: &mut EntitySnapshot,
    ctx: &Context<'_>,
    effects: &mut Effects,
) -> Result<(), String> {
    match damage(actor, ctx)? {
        0 => {
            movement(actor, ctx, effects);
            if actor.phase & 7 == 0 {
                let destination = point(route(actor, ctx)?)?;
                if actor.control_flags & 0x40000 == 0 {
                    aim_roll(actor, destination);
                }
                avoid(actor, ctx, true)?;
            }
        }
        1 => hit_reaction(actor, ctx)?,
        2 => actor.set_source_action(28),
        _ => unreachable!(),
    }
    Ok(())
}

fn guard(
    actor: &mut EntitySnapshot,
    ctx: &Context<'_>,
    effects: &mut Effects,
) -> Result<(), String> {
    let destination = point(route(actor, ctx)?)?;
    if (i32::from(destination.x as i16) - i32::from(actor.position.x as i16)).abs() > 3072
        || (i32::from(destination.y as i16) - i32::from(actor.position.y as i16)).abs() > 3072
    {
        point_step(actor, ctx, effects)?;
    } else {
        wander(actor, ctx, effects)?;
    }
    if actor.action != 31
        || actor.record[77] != 2
        || actor.record[61] == 0
        || actor.phase % CADENCE != 0
    {
        return Ok(());
    }
    let mut selected = None;
    let mut best = u32::MAX;
    for &slot in ctx.class3 {
        let Some(target) = current(actor, ctx, slot)? else {
            continue;
        };
        let dx = i32::from(target.position.x) - i32::from(actor.position.x);
        let dy = i32::from(target.position.y.wrapping_sub(actor.position.y) as i16);
        let squared = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32;
        if squared <= RANGE.wrapping_mul(RANGE)
            && target.control_flags & 0x20 == 0
            && squared < best
            && angle_distance(
                i32::from(actor.yaw),
                heading_for_positions(actor.position, target.position),
            ) < HALF_ANGLE
        {
            best = squared;
            selected = Some(slot);
        }
    }
    if let Some(slot) = selected {
        put(actor, 154, slot);
        actor.record[77] = 10;
        attack(actor)?;
    }
    Ok(())
}

fn bind_leader(
    actor: &EntitySnapshot,
    ctx: &Context<'_>,
    stage: RuntimeStageVariable,
) -> Result<u16, String> {
    if stage.flags & 1 != 0 {
        for &slot in &ctx.models[3] {
            let Some(peer) = current(actor, ctx, slot)? else {
                continue;
            };
            if required(slot, peer.relations.stage_index, "peer StageVar index")?
                == actor.record[76]
            {
                let relation = required(
                    slot,
                    peer.relations.stage_relation,
                    "peer StageVar relation",
                )?;
                if relation != 0 {
                    return Ok(relation);
                }
            }
        }
    }
    let model = match stage.relation {
        RuntimeRelation::AxisScalar(value) | RuntimeRelation::Scalar(value) => value as u16,
        _ => return Err("Worm family StageVar has no source model selector".into()),
    };
    let slots = ctx
        .models
        .get(usize::from(model))
        .ok_or_else(|| format!("Worm StageVar model{model} is outside the source family table"))?;
    let mut selected = 0;
    let mut best = u32::MAX;
    for &slot in slots {
        let Some(target) = current(actor, ctx, slot)? else {
            continue;
        };
        let squared = distance_xy(actor.position, target.position);
        if squared < best {
            best = squared;
            selected = slot;
        }
    }
    Ok(selected)
}

fn leader(
    actor: &mut EntitySnapshot,
    ctx: &Context<'_>,
    stage: RuntimeStageVariable,
    bind: bool,
) -> Result<Option<source_target::View>, String> {
    let slot = if stage.flags & 2 != 0 {
        if word(actor, 78) == 0 && bind {
            let relation = bind_leader(actor, ctx, stage)?;
            put(actor, 78, relation);
        }
        word(actor, 78)
    } else {
        match stage.relation {
            RuntimeRelation::LiveEntity(slot) | RuntimeRelation::LiveEntityIndex(slot) => slot,
            _ => 0,
        }
    };
    current(actor, ctx, slot)
}

fn follow(
    actor: &mut EntitySnapshot,
    ctx: &Context<'_>,
    mode: u8,
    effects: &mut Effects,
) -> Result<(), String> {
    match damage(actor, ctx)? {
        0 => {
            movement(actor, ctx, effects);
            if actor.phase & 7 == 0 {
                let stage = route(actor, ctx)?;
                if let Some(target) = leader(actor, ctx, stage, true)? {
                    if actor.control_flags & 0x40000 == 0 {
                        aim_roll(actor, target.position);
                    }
                    avoid(actor, ctx, false)?;
                    let target = current(actor, ctx, target.slot)?
                        .expect("current leader remains addressed");
                    let width = i32::from(actor.array_pitch())
                        + i32::from(
                            required(target.slot, target.collision, "leader collision")?.half_width,
                        );
                    if (i32::from(actor.position.x) - i32::from(target.position.x)).abs() < width
                        && (i32::from(actor.position.y) - i32::from(target.position.y)).abs()
                            < width
                    {
                        actor.set_source_roll(
                            heading_for_positions(target.position, actor.position) as i16,
                        );
                    }
                } else if actor.phase & 15 == 0 && actor.control_flags & 0x40000 == 0 {
                    wander_roll(actor);
                }
            }
        }
        1 => hit_reaction(actor, ctx)?,
        2 => actor.set_source_action(28),
        _ => unreachable!(),
    }
    // D7C0/D700 tails run even if D8C0 selected prekill or changed StageVar2.
    if mode != 5 && actor.phase & 7 == 0 {
        let stage = route(actor, ctx)?;
        if let Some(leader) = leader(actor, ctx, stage, false)? {
            let target = if mode == 3 {
                Some(leader)
            } else {
                let slot = required(leader.slot, leader.relations.target, "leader target")?;
                if slot == 0xae02 {
                    None
                } else {
                    current(actor, ctx, slot)?
                }
            };
            if let Some(target) = target
                && distance(actor.position, target.position) <= RANGE
            {
                put(actor, 154, target.slot);
                actor.record[77] = 10;
                attack(actor)?;
            }
        }
    }
    Ok(())
}

fn parent_pose(actor: &mut EntitySnapshot, ctx: &Context<'_>) -> Result<(), String> {
    let parent = current(actor, ctx, word(actor, 44))?;
    if parent.is_none_or(|parent| parent.life < 0) {
        actor.disable_drawing();
    }
    let parent = parent.unwrap_or_else(|| sentinel(ctx));
    let position = Position {
        z: parent.position.z.max(0),
        ..parent.position
    };
    ctx.terrain.set_source_predicted_position(position);
    actor.set_position(position);
    actor.set_yaw(required(parent.slot, parent.yaw, "parent yaw")?);
    actor.set_source_roll(required(parent.slot, parent.roll, "parent roll")?);
    Ok(())
}

fn escort(
    actor: &mut EntitySnapshot,
    ctx: &Context<'_>,
    effects: &mut Effects,
) -> Result<(), String> {
    match damage(actor, ctx)? {
        0 => {
            movement(actor, ctx, effects);
            if actor.phase & 7 == 0
                && let Some(target) = current(actor, ctx, word(actor, 154))?
            {
                if actor.control_flags & 0x40000 == 0 {
                    aim_roll(actor, target.position);
                }
                avoid(actor, ctx, false)?;
            }
        }
        1 => {
            movement(actor, ctx, effects);
            let attacker = word(actor, 42);
            let parent = word(actor, 44);
            if attacker != parent && different_family(actor, ctx, attacker)? {
                put(actor, 154, attacker);
                attack(actor)?;
                effects.spell_awards.push((parent, 19));
            }
        }
        2 => put(actor, 50, 1),
        _ => unreachable!(),
    }
    Ok(())
}

fn summon(
    actor: &mut EntitySnapshot,
    ctx: &Context<'_>,
    mode: u8,
    effects: &mut Effects,
) -> Result<(), String> {
    let parent_slot = word(actor, 44);
    let parent = current(actor, ctx, parent_slot)?
        .filter(|parent| parent.life >= 0 && parent.control_flags & 0x400 == 0);
    if parent.is_none() {
        put(actor, 50, 0);
    } else if mode == 13 {
        let remaining = word(actor, 50).wrapping_sub(1);
        put(actor, 50, remaining);
    }
    if (word(actor, 50) as i16) <= 0 {
        effects.expiry = Some(mode == 13);
        return Ok(());
    }
    let parent = parent.expect("source summon parent checked");
    let mut target = current(actor, ctx, word(actor, 154))?;
    if target.is_some_and(|target| target.life <= 0 || target.control_flags & 0x400 != 0) {
        put(actor, 154, 0);
        target = None;
    }
    if target.is_none() && actor.phase & 7 == 0 {
        let mut nearest = u32::MAX;
        for &slot in ctx.class3 {
            let Some(candidate) = current(actor, ctx, slot)? else {
                continue;
            };
            if candidate.source_id != parent.source_id && matches!(candidate.model, 0 | 1) {
                let squared = distance_xy(parent.position, candidate.position);
                if squared < nearest {
                    nearest = squared;
                    target = Some(candidate);
                }
            }
        }
        put(actor, 154, target.map_or(0, |target| target.slot));
    }
    if let Some(target) = target {
        escort(actor, ctx, effects)?;
        if actor.phase & 7 == 0
            && let Some(target) = current(actor, ctx, target.slot)?
            && distance(actor.position, target.position) < RANGE
        {
            actor.set_source_action(26);
        }
    } else {
        put(actor, 154, parent_slot);
        escort(actor, ctx, effects)?;
        put(actor, 154, 0);
        let remaining = word(actor, 50).wrapping_sub(4);
        put(actor, 50, remaining);
    }
    Ok(())
}

fn blink(actor: &mut EntitySnapshot, ctx: &mut Context<'_>, clear: bool) -> Result<(), String> {
    let remaining = word(actor, 50) as i16;
    // Source word30 is unsigned; its subtraction is promoted before compare.
    let duration = i32::from(word(actor, 52));
    let phase = if clear {
        0
    } else if i32::from(remaining) <= duration - 16 && remaining >= 16 {
        if actor.phase & 0x1c == 16 {
            actor.phase & 3
        } else {
            0
        }
    } else if actor.phase & 1 != 0 {
        0
    } else {
        2
    };
    let bits = match phase {
        1 | 3 => 0x0200_0000,
        2 => 0x0400_0000,
        _ => 0,
    };
    actor.set_control_flags((actor.control_flags & !0x0600_0000) | bits);
    super::super::super::level2_class5::firefly::alliance_blink_chain(
        ctx.arena,
        word(actor, 56),
        bits,
        ctx.targets,
    )
}

fn alliance(
    actor: &mut EntitySnapshot,
    ctx: &mut Context<'_>,
    effects: &mut Effects,
) -> Result<(), String> {
    let remaining = word(actor, 50).wrapping_sub(1);
    put(actor, 50, remaining);
    if (word(actor, 50) as i16) > 0 {
        blink(actor, ctx, false)?;
    }
    let parent_slot = word(actor, 44);
    let parent = if (word(actor, 50) as i16) > 0 {
        current(actor, ctx, parent_slot)?.filter(|parent| parent.life > 0)
    } else {
        None
    };
    if parent.is_none() {
        actor.record[77] = 10;
        put(actor, 154, 0);
        put(actor, 44, 0);
        actor.record[76] = word(actor, 78) as u8;
        blink(actor, ctx, true)?;
        return Ok(());
    }
    let saved = word(actor, 154);
    put(actor, 154, parent_slot);
    escort(actor, ctx, effects)?;
    let parent = current(actor, ctx, parent_slot)?.expect("current Alliance parent checked");
    let target = required(parent_slot, parent.relations.target, "parent target")?;
    let target = if target == 0 {
        required(parent_slot, parent.relations.attacker, "parent attacker")?
    } else {
        target
    };
    put(actor, 154, if target == 0 { saved } else { target });
    if let Some(target) = current(actor, ctx, word(actor, 154))? {
        let sibling = required(target.slot, target.relations.parent, "target parent")?
            == parent_slot
            && required(
                target.slot,
                target.relations.stage_mode,
                "target StageVar mode",
            )? == 14;
        if sibling || target.life <= 0 || target.control_flags & 0x400 != 0 {
            put(actor, 154, 0);
        } else if distance(actor.position, target.position) < RANGE {
            actor.set_source_action(26);
            effects.spell_awards.push((parent_slot, 24));
        }
    }
    Ok(())
}

pub(crate) fn step(
    arena: &mut SourceArena,
    runtime: &RuntimeSnapshot,
    terrain: &SessionTerrain,
    actor: &mut EntitySnapshot,
    models: &[Vec<u16>; 29],
    class3: &[u16],
    targets: source_target::Context<'_>,
) -> Result<Effects, String> {
    if (actor.class, actor.model, actor.action) != (5, 3, 31) {
        return Err(format!(
            "Worm stage rejects current {}/{}/{} at{}",
            actor.class, actor.model, actor.action, actor.index
        ));
    }
    let mut ctx = Context {
        arena,
        runtime,
        terrain,
        models,
        class3,
        targets,
    };
    let mut effects = Effects::default();
    match actor.record[77] {
        1 => point_step(actor, &ctx, &mut effects)?,
        2 => guard(actor, &ctx, &mut effects)?,
        mode @ 3..=5 => follow(actor, &ctx, mode, &mut effects)?,
        6..=9 => wander(actor, &ctx, &mut effects)?,
        10 => attack(actor)?,
        12 => parent_pose(actor, &ctx)?,
        mode @ (13 | 16) => summon(actor, &ctx, mode, &mut effects)?,
        14 => alliance(actor, &mut ctx, &mut effects)?,
        17 => {
            movement(actor, &ctx, &mut effects);
            if actor.life < 0 {
                actor.disable_drawing();
            } else if let Some(target) = current(actor, &ctx, word(actor, 154))?
                .filter(|target| target.life >= 0 && target.control_flags & 0x400 == 0)
            {
                actor
                    .set_source_roll(heading_for_positions(actor.position, target.position) as i16);
                actor.set_actual_speed(actor.actual_speed.wrapping_sub(8));
                if actor.actual_speed <= 16 {
                    actor.record[77] = 16;
                }
            } else {
                actor.record[77] = 16;
            }
        }
        // Source1D5D0 has literal returns for all other mode bytes.
        _ => {}
    }
    Ok(effects)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (
        SourceArena,
        RuntimeSnapshot,
        SessionTerrain,
        EntitySnapshot,
        u16,
        Vec<crate::native_world::ParticleParameter>,
    ) {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut terrain = session.canonical_terrain().clone();
        let runtime = session.runtime().clone();
        let player = session.state.entity_index;
        let particles = session.session.definition.presentation.particles.to_vec();
        let mut arena = session.source_arena().clone();
        let root = super::super::dynamic::construct_authored(
            &mut arena,
            &mut terrain,
            65_000,
            session.state.position,
            9000,
            &particles,
        )
        .expect("configured control uses the actual root+16 constructor");
        let mut actor =
            super::super::dynamic::current_root_packet(&arena, root.source_entity_index).unwrap();
        actor.set_source_action(31);
        actor.set_control_flags(actor.control_flags | 0x800);
        actor.phase = 8;
        actor.record[66] = 8;
        (arena, runtime, terrain, actor, player, particles)
    }

    #[test]
    fn configured_dispatch_noops_attack_and_fatal_preserve_source_tails() {
        let (mut arena, runtime, terrain, actor, player, _) = fixture();
        let models = std::array::from_fn(|_| Vec::new());
        for mode in [0, 11, 15, 18, 255] {
            let mut row = actor.clone();
            row.record[76] = 255;
            row.record[77] = mode;
            let before = row.record.clone();
            let effects = step(
                &mut arena,
                &runtime,
                &terrain,
                &mut row,
                &models,
                &[],
                source_target::Context::default(),
            )
            .unwrap();
            assert_eq!(row.record, before);
            assert_eq!(effects.population_decrements, 0);
            assert_eq!(effects.expiry, None);
        }
        let mut row = actor.clone();
        row.record[76] = 255;
        row.record[77] = 10;
        put(&mut row, 154, 777);
        step(
            &mut arena,
            &runtime,
            &terrain,
            &mut row,
            &models,
            &[],
            source_target::Context::default(),
        )
        .unwrap();
        assert_eq!((row.action, word(&row, 154)), (26, 777));

        let mut row = actor;
        row.record[76] = 255;
        row.record[77] = 6;
        row.record[98..102].copy_from_slice(&9001_i32.to_le_bytes());
        put(&mut row, 102, player);
        let effects = step(
            &mut arena,
            &runtime,
            &terrain,
            &mut row,
            &models,
            &[],
            source_target::Context::default(),
        )
        .unwrap();
        assert_eq!(
            (row.life, row.action, word(&row, 40), word(&row, 102)),
            (-1, 28, player, 0)
        );
        assert_eq!(
            effects.population_decrements, 0,
            "fatal mailbox is not source movement death"
        );
        assert_eq!(
            i32::from_le_bytes(row.record[98..102].try_into().unwrap()),
            9001
        );
    }

    #[test]
    fn configured_follow_outer_tail_runs_after_its_fatal_prelude() {
        let (mut arena, mut runtime, terrain, mut actor, player, _) = fixture();
        actor.set_position(arena.entities[usize::from(player)].position);
        actor.record[76] = 0;
        actor.record[77] = 3;
        runtime.stage_variables[0].flags = 0;
        runtime.stage_variables[0].relation = RuntimeRelation::LiveEntity(player);
        actor.record[98..102].copy_from_slice(&9001_i32.to_le_bytes());
        put(&mut actor, 102, player);
        let models = std::array::from_fn(|_| Vec::new());
        let effects = step(
            &mut arena,
            &runtime,
            &terrain,
            &mut actor,
            &models,
            &[],
            source_target::Context::default(),
        )
        .unwrap();
        assert_eq!(
            (
                actor.life,
                actor.action,
                actor.record[77],
                word(&actor, 154)
            ),
            (-1, 26, 10, player)
        );
        assert_eq!(effects.population_decrements, 0);
    }

    #[test]
    fn configured_current_leader_separation_follows_peer_avoidance() {
        let (mut arena, mut runtime, mut terrain, mut actor, _, particles) = fixture();
        let position = actor.position;
        let actor_rng = actor.entity_rng;
        let other = super::super::dynamic::construct_authored(
            &mut arena,
            &mut terrain,
            65_001,
            position,
            9000,
            &particles,
        )
        .unwrap();
        let handle = arena.handle(other.source_entity_index).unwrap();
        let mut peer =
            super::super::dynamic::current_root_packet(&arena, other.source_entity_index).unwrap();
        peer.set_position(Position {
            x: position.x.wrapping_add(1),
            y: position.y.wrapping_add(2),
            ..position
        });
        assert!(super::super::dynamic::commit_root_packet(&mut arena, handle, &peer).unwrap());
        // The leader is a current birth after the supplied frozen peer list.
        let leader = super::super::dynamic::construct_authored(
            &mut arena,
            &mut terrain,
            65_002,
            position,
            9000,
            &particles,
        )
        .unwrap();
        let leader_handle = arena.handle(leader.source_entity_index).unwrap();
        let mut leader =
            super::super::dynamic::current_root_packet(&arena, leader.source_entity_index).unwrap();
        leader.set_position(Position {
            x: position.x.wrapping_add(2),
            y: position.y.wrapping_add(1),
            ..position
        });
        assert!(
            super::super::dynamic::commit_root_packet(&mut arena, leader_handle, &leader).unwrap()
        );
        // Poison the compatibility projection; only the typed current pose is authoritative.
        arena.entities[usize::from(leader_handle.source_slot)].set_position(Position {
            x: 100,
            y: 100,
            z: -100,
        });
        actor.record[76] = 0;
        actor.record[77] = 5;
        runtime.stage_variables[0].flags = 0;
        runtime.stage_variables[0].relation =
            RuntimeRelation::LiveEntity(leader_handle.source_slot);
        let mut models = std::array::from_fn(|_| Vec::new());
        models[3] = vec![handle.source_slot];
        step(
            &mut arena,
            &runtime,
            &terrain,
            &mut actor,
            &models,
            &[],
            source_target::Context::default(),
        )
        .unwrap();
        assert_eq!(
            actor.source_roll(),
            heading_for_positions(leader.position, actor.position) as i16
        );
        assert_ne!(
            actor.source_roll(),
            heading_for_positions(peer.position, actor.position) as i16,
            "the independent leader separation follows and replaces peer avoidance"
        );
        assert_eq!(
            actor.entity_rng, actor_rng,
            "phase8 does not draw jitter RNG"
        );
    }

    #[test]
    fn configured_parent_pose_uses_current_sentinel_after_hide_and_expiry_is_terminal() {
        let (mut arena, runtime, terrain, mut actor, _, _) = fixture();
        arena.entities[0].set_position(Position {
            x: 1234,
            y: 5678,
            z: -4,
        });
        arena.entities[0].set_yaw(-123);
        arena.entities[0].set_source_roll(456);
        actor.record[76] = 255;
        actor.record[77] = 12;
        put(&mut actor, 44, 0);
        let models = std::array::from_fn(|_| Vec::new());
        step(
            &mut arena,
            &runtime,
            &terrain,
            &mut actor,
            &models,
            &[],
            source_target::Context::default(),
        )
        .unwrap();
        assert!(actor.drawing_disabled());
        assert_eq!(
            actor.position,
            Position {
                x: 1234,
                y: 5678,
                z: 0
            }
        );
        assert_eq!((actor.yaw, actor.source_roll()), (-123, 456));
        for mode in [13, 16] {
            actor.record[77] = mode;
            put(&mut actor, 50, 100);
            let effects = step(
                &mut arena,
                &runtime,
                &terrain,
                &mut actor,
                &models,
                &[],
                source_target::Context::default(),
            )
            .unwrap();
            assert_eq!(word(&actor, 50), 0);
            assert_eq!(effects.expiry, Some(mode == 13));
        }
    }

    #[test]
    fn configured_alliance_blink_writes_real_bodies_with_unsigned_duration() {
        let (mut arena, runtime, terrain, mut actor, player, _) = fixture();
        actor.record[77] = 14;
        actor.phase = 17;
        actor.record[66] = 17;
        put(&mut actor, 44, player);
        put(&mut actor, 50, 40);
        put(&mut actor, 52, 32768);
        put(&mut actor, 154, 0);
        let models = std::array::from_fn(|_| Vec::new());
        let effects = step(
            &mut arena,
            &runtime,
            &terrain,
            &mut actor,
            &models,
            &[],
            source_target::Context::default(),
        )
        .unwrap();
        assert_eq!(word(&actor, 50), 39);
        assert_eq!(actor.control_flags & 0x0600_0000, 0x0200_0000);
        assert!(
            arena
                .economy
                .authored_class5_model3_children()
                .iter()
                .filter(|body| body.parent_entity_index == actor.index)
                .all(|body| body.control_flags & 0x0600_0000 == 0x0200_0000)
        );
        assert_eq!(effects.expiry, None);
    }
}
