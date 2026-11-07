//! Complete reachable class-5/model-3 constructor and action family.
//!
//! The Level 2 model-3 carrier is the `sub_4B6F0` root (action 25) and its
//! sixteen action-232 presentation children.  The model-27 carrier from
//! `sub_4D000` is a separate source family and is deliberately absent here.

use crate::{
    FlightWorld, Position, Tile, angle_distance, heading_for_positions,
    campaign::source_arena::{SourceArena, SourceEntityHandle},
    native_world::{
        ENTITY_DRAW_DISABLED_BIT, EntitySnapshot, FireballProjectileOrigin,
        Level2AuthoredClass5Model3Child, Level2AuthoredClass5Model3State,
        Level2AuthoredClass5Source, Level2DynamicArenaKind, Level2EconomyEvent,
        Level2EconomyState, MODEL3_RAW_SOURCE_DISABLED_BIT, ParticleParameter,
        RuntimeSnapshot, SessionTerrain, level2_dynamic_economy, level2_next_rng16,
        level2_wrapping_source_distance_3d,
    },
    renderable_map_chain::RenderableMapSeed,
};

const MODEL3_DESCRIPTOR_INDEX: u16 = 74;
const MODEL3_TARGET_RANGE: u32 = crate::level_definition::WORM_PROPERTIES.target_range;
const MODEL3_TARGET_HALF_ANGLE: i16 = crate::level_definition::WORM_PROPERTIES.target_half_angle;
const MODEL3_DESCRIPTOR_FLAGS: u8 = crate::level_definition::WORM_PROPERTIES.behavior_flags;
const MODEL3_DESCRIPTOR_CADENCE: u8 = crate::level_definition::WORM_PROPERTIES.scan_period;
const MODEL3_CHILD_COUNT: usize = crate::level_definition::WORM_PROPERTIES.body_count;
#[derive(Clone, Copy, Default)]
struct Model3PartGeometry { yaw: i16, axis: i16, fov: i16, spacing: i16 }

/// Source constructor geometry, validated before the first NewEvent.
pub(in crate::native_world) struct Model3Geometry {
    root_yaw: i16,
    root_axis: i16,
    root_fov: i16,
    children: [Model3PartGeometry; MODEL3_CHILD_COUNT],
}

pub(in crate::native_world) fn model3_geometry(particles: &[ParticleParameter]) -> Option<Model3Geometry> {
    model3_geometry_with_properties(particles, crate::level_definition::WORM_PROPERTIES)
}

fn model3_geometry_with_properties(
    particles: &[ParticleParameter], properties: crate::level_definition::WormProperties,
) -> Option<Model3Geometry> {
    let root = particles.get(88)?;
    let mut children = [Model3PartGeometry::default(); MODEL3_CHILD_COUNT];
    for (index, child) in children.iter_mut().enumerate() {
        let particle = particles.get(89 + index)?;
        let axis = (u32::from(particle.speed) * u32::from(properties.body_scale_percent) / 100) as i16;
        *child = Model3PartGeometry {
            yaw: (particle.rotation_speed / 2) as i16,
            axis,
            fov: (u32::from(particle.rotation_speed) * u32::from(properties.body_scale_percent) / 100) as i16,
            spacing: if index == 0 { (u32::from(axis as u16) * u32::from(properties.first_body_spacing_percent) / 100) as i16 } else { axis },
        };
    }
    Some(Model3Geometry {
        root_yaw: (root.rotation_speed / 2) as i16,
        root_axis: (u32::from(root.speed) * u32::from(properties.root_scale_percent) / 100) as i16,
        root_fov: (u32::from(root.rotation_speed) * u32::from(properties.root_scale_percent) / 100) as i16,
        children,
    })
}

// Descriptor row 74 (`str_D7BD6[74]`) is passed to `sub_580E0` by both
// `sub_1B8C0` and `sub_1EEE0`: ground offset 0 and a -64 approach step.
const MODEL3_SOURCE_SETTLE_STEP: i16 = crate::level_definition::WORM_PROPERTIES.descent;
const MODEL3_SOURCE_SLOPE_LIMIT: i32 = crate::level_definition::WORM_PROPERTIES.maximum_slope;
const MODEL3_ALLOWED_MATERIAL_MASK: u32 = crate::level_definition::WORM_PROPERTIES.terrain_mask;
const MODEL3_SOURCE_CAVE_CLEARANCE: i32 = crate::level_definition::WORM_PROPERTIES.ground_clearance as i32;
const MODEL3_RAW_MOTION_COLLISION_BIT: u32 = 0x0004_0000;
const MODEL3_ACTION26_ARRAY_PITCH: i16 = 1;

fn model3_distance_squared_2d(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    dx.wrapping_mul(dx)
        .wrapping_add(dy.wrapping_mul(dy))
        .cast_unsigned()
}

fn model3_slope_measure<W: FlightWorld>(position: Position, world: &W) -> i32 {
    let tile_x = position.x.to_be_bytes()[0];
    let tile_y = position.y.to_be_bytes()[0];
    let height = |x: u8, y: u8| {
        i32::from(world.ground_height(Position {
            x: u16::from(x) << 8,
            y: u16::from(y) << 8,
            z: 0,
        })) >> 5
    };
    let point_1 = height(tile_x, tile_y);
    let point_2 = height(tile_x.wrapping_add(1), tile_y);
    let point_3 = height(tile_x.wrapping_add(1), tile_y.wrapping_add(1));
    let point_4 = height(tile_x, tile_y.wrapping_add(1));
    let diagonal_1 = (point_1 + point_4 - point_2 - point_3).abs();
    let diagonal_2 = (point_1 + point_2 - point_4 - point_3).abs();
    diagonal_1.max(diagonal_2)
}

fn model3_sub102d0_probe_blocked<W: FlightWorld>(
    model3: &Level2AuthoredClass5Model3State,
    candidate: Position,
    heading: i16,
    world: &W,
) -> bool {
    let maximum_extent = i32::from(model3.render_pitch.max(model3.render_roll)).max(0);
    let mut probe = candidate;
    let mut distance = 0;
    while distance <= maximum_extent {
        if world.material_mask(probe) & !MODEL3_ALLOWED_MATERIAL_MASK != 0 {
            return true;
        }
        if world.cave_collision_blocked(probe) {
            return true;
        }
        if let Some(ceiling) = world.ceiling_height(probe) {
            let tile = Tile {
                x: probe.x.to_be_bytes()[0],
                y: probe.y.to_be_bytes()[0],
            };
            if world.angle_flags(tile) & 8 != 0
                || i32::from(model3.render_fov)
                    + i32::from(world.ground_height(probe))
                    + MODEL3_SOURCE_CAVE_CLEARANCE
                    > i32::from(ceiling)
            {
                return true;
            }
        }
        distance += 256;
        crate::move_entity(&mut probe, heading, 0, 256);
    }
    false
}

fn model3_candidate_blocked<W: FlightWorld>(
    model3: &Level2AuthoredClass5Model3State,
    candidate: Position,
    heading: i16,
    world: &W,
) -> bool {
    model3_sub102d0_probe_blocked(model3, candidate, heading, world)
        || model3_slope_measure(candidate, world) >= MODEL3_SOURCE_SLOPE_LIMIT
}

fn model3_source_settle_position<W: FlightWorld>(position: &mut Position, world: &W) {
    let ground = world.ground_height(*position);
    if position.z > ground {
        position.z = position.z.wrapping_add(MODEL3_SOURCE_SETTLE_STEP);
    }
    if position.z <= ground {
        position.z = ground;
    }
}

fn model3_turn_delta(from: i16, to: i16) -> i16 {
    let from = i32::from(from) & 0x07ff;
    let to = i32::from(to) & 0x07ff;
    let mut delta = to - from;
    if delta.abs() > 1024 {
        delta += if delta >= 0 { -2048 } else { 2048 };
    }
    // Descriptor 74's nominal turn word is 34, but sub_58350 ignores that
    // argument and clamps by its subtype argument, which is 5.
    let magnitude = delta.abs().min(i32::from(crate::level_definition::WORM_PROPERTIES.turn_response));
    i16::try_from(magnitude * delta.signum()).expect("model-3 turn delta fits i16")
}

fn model3_heading_after_turn(heading: i16, roll: i16) -> i16 {
    i16::try_from((i32::from(heading) + i32::from(model3_turn_delta(heading, roll))) & 0x07ff)
        .expect("masked model-3 heading fits i16")
}

fn model3_source_retry_headings(yaw: i16) -> [i16; 3] {
    let original = u16::from_ne_bytes(yaw.to_ne_bytes());
    let second = original.wrapping_add(341) & 0x07ff;
    let third = (original.wrapping_sub(85) & 0x00ff) | ((original.wrapping_sub(341) >> 8 & 7) << 8);
    let fourth = (original.wrapping_add(0x0400) & 0x0700) | (original & 0x00ff);
    [
        i16::try_from(second).expect("model-3 retry heading fits"),
        i16::try_from(third).expect("model-3 retry heading fits"),
        i16::try_from(fourth).expect("model-3 retry heading fits"),
    ]
}

#[derive(Clone, Copy, Debug)]
struct Model3Peer {
    source_entity_index: u16,
    position: Position,
    life: i32,
    maximum_speed: i16,
    actual_speed: i16,
    action: u8,
    word_0x96: u16,
    word_0x32: u16,
    drawing_enabled: bool,
}

pub(crate) fn alliance_projection(arena: &SourceArena, slot: u16) -> Option<EntitySnapshot> {
    let handle = arena.handle(slot)?;
    if !matches!(arena.resolve(handle), Some(crate::campaign::source_arena::SourceEntityView::Dynamic(Level2DynamicArenaKind::AuthoredClass5))) { return None; }
    let source = arena.economy.authored_class5.iter().find(|row| row.active && row.source_entity_index == slot && row.model == 3)?;
    let state = arena.economy.authored_class5_model3.iter().find(|row| row.source_entity_index == slot)?;
    let mut row = arena.entities.get(usize::from(slot))?.clone();
    row.active = true; row.record[2] = 1; row.source_id = state.entity_id;
    row.record[30..32].copy_from_slice(&state.entity_id.to_le_bytes());
    row.class = 5; row.model = 3; row.record[67] = 5; row.record[68] = 3;
    row.set_source_action(source.action); row.set_position(source.position); row.set_life(source.life);
    row.set_max_life(source.max_life); row.set_mana(source.mana);
    row.set_yaw(source.yaw); row.set_pitch(source.pitch); row.set_source_roll(source.roll); row.set_fov(source.fov);
    row.set_actual_speed(source.actual_speed); row.set_entity_rng(source.entity_rng);
    row.set_source_dword_10(source.dword_0x10); row.set_phase(source.source_byte_3e);
    row.set_subspell_index(source.subspell_index);
    row.record[40..42].copy_from_slice(&source.word_0x24.to_le_bytes());
    row.record[42..44].copy_from_slice(&source.word_0x26.to_le_bytes());
    row.record[48..50].copy_from_slice(&state.word_0x2c.to_le_bytes());
    row.set_control_flags((state.control_flags & !0x400) | if source.drawing_enabled { 0 } else { 0x400 });
    row.record[44..46].copy_from_slice(&state.parent_entity_index.to_le_bytes());
    row.record[50..52].copy_from_slice(&state.stage_remaining.to_le_bytes());
    row.record[52..54].copy_from_slice(&state.alliance_duration.to_le_bytes());
    row.record[54..56].copy_from_slice(&state.word_0x32.to_le_bytes());
    row.record[56..58].copy_from_slice(&state.word_0x34.to_le_bytes());
    row.record[58..60].copy_from_slice(&state.word_0x36.to_le_bytes());
    row.record[61] = source.source_byte_39; row.record[63] = source.byte_0x3b;
    row.record[86..88].copy_from_slice(&state.render_yaw.to_le_bytes());
    row.record[88..90].copy_from_slice(&state.render_pitch.to_le_bytes());
    row.record[90..92].copy_from_slice(&state.render_roll.to_le_bytes());
    row.record[92..94].copy_from_slice(&state.render_fov.to_le_bytes());
    row.record[98..102].copy_from_slice(&state.damage_dword_0x5e_94.to_le_bytes());
    row.record[102..104].copy_from_slice(&source.pending_word_0x62.to_le_bytes());
    row.record[136..138].copy_from_slice(&source.minimum_speed.to_le_bytes());
    row.record[138..140].copy_from_slice(&source.maximum_speed.to_le_bytes());
    row.parameter_relation = Some(state.descriptor_index);
    row.particle_parameter_index = Some(source.particle_index);
    row.record[76] = state.stage_var1; row.record[77] = state.stage_var2;
    row.record[78..80].copy_from_slice(&state.stage_relation.to_le_bytes());
    row.record[154..156].copy_from_slice(&source.word_0x96_150.to_le_bytes());
    Some(row)
}

/// Project the current Worm root for source callbacks. Typed roots use their
/// owning state; imported roots use their current source record.
pub(super) fn current_root_packet(arena: &SourceArena, slot: u16) -> Option<EntitySnapshot> {
    if let Some(row) = alliance_projection(arena, slot) {
        return Some(row);
    }
    if arena.economy.live_arena_kind(slot).is_some() {
        return None;
    }
    arena.entities.get(usize::from(slot)).filter(|row|
        row.index == slot && row.active && (row.class, row.model) == (5, 3)
    ).cloned()
}

/// Live typed Worm sprites. Imported roots/parts retain their ordinary raw
/// presentation path; dynamic slots must be projected from their actual owner.
pub(crate) fn presentation_candidates(
    arena: &SourceArena,
) -> impl Iterator<Item = (u16, u16, u16, i16, Position, u32)> + '_ {
    let roots = arena.economy.authored_class5.iter()
        .filter(|source| source.active && source.model == 3)
        .filter_map(|source| current_root_packet(arena, source.source_entity_index))
        .filter(|row| row.control_flags & 0x421 == 0)
        .map(|row| (
            row.index, row.source_id,
            row.particle_parameter_index.expect("typed Worm root owns its particle"),
            row.yaw, row.position, row.control_flags,
        ));
    let parts = arena.economy.authored_class5_model3_children.iter()
        .filter(|part| part.active && part.drawing_enabled && part.control_flags & 0x421 == 0)
        .map(|part| (
            part.source_entity_index,
            // sub4B6F0 qmemcpy retains the root NewEvent ID before any later
            // owner change. This is the retained birth slot, not a lookup of
            // whatever now occupies the parent address.
            part.parent_entity_index,
            part.particle_index, part.yaw, part.position, part.control_flags,
        ));
    roots.chain(parts)
}

/// Commit a callback-local root before any source allocation can replace its
/// fixed address. A typed root never writes the retired raw record beneath it.
pub(super) fn commit_root_packet(
    arena: &mut SourceArena,
    handle: SourceEntityHandle,
    row: &EntitySnapshot,
) -> std::result::Result<bool, String> {
    let slot = handle.source_slot;
    if row.index != slot || arena.handle(slot) != Some(handle) { return Ok(false); }
    if alliance_projection(arena, slot).is_some() {
        let source = arena.economy.authored_class5.iter_mut().find(|source|
            source.active && source.source_entity_index == slot && source.model == 3
        ).ok_or_else(|| format!("Worm {slot} lost its typed root"))?;
        source.action = row.action; source.position = row.position; source.life = row.life;
        source.max_life = row.max_life; source.mana = row.mana; source.entity_rng = row.entity_rng;
        source.yaw = row.yaw; source.pitch = row.pitch; source.roll = row.source_roll(); source.fov = row.fov;
        source.actual_speed = row.actual_speed; source.dword_0x10 = row.source_dword_10();
        source.word_0x24 = u16::from_le_bytes([row.record[40], row.record[41]]);
        source.word_0x26 = u16::from_le_bytes([row.record[42], row.record[43]]);
        source.pending_word_0x62 = u16::from_le_bytes([row.record[102], row.record[103]]);
        source.word_0x96_150 = row.source_word_96();
        source.source_byte_39 = row.record[61]; source.source_byte_3e = row.phase;
        source.drawing_enabled = !row.drawing_disabled();
        let state = arena.economy.authored_class5_model3.iter_mut().find(|state|
            state.source_entity_index == slot
        ).ok_or_else(|| format!("Worm {slot} lost its typed state"))?;
        state.control_flags = row.control_flags;
        state.parent_entity_index = u16::from_le_bytes([row.record[44], row.record[45]]);
        state.word_0x2c = i16::from_le_bytes([row.record[48], row.record[49]]);
        state.stage_remaining = i16::from_le_bytes([row.record[50], row.record[51]]);
        state.alliance_duration = u16::from_le_bytes([row.record[52], row.record[53]]);
        state.word_0x32 = u16::from_le_bytes([row.record[54], row.record[55]]);
        state.word_0x34 = u16::from_le_bytes([row.record[56], row.record[57]]);
        state.word_0x36 = i16::from_le_bytes([row.record[58], row.record[59]]);
        state.stage_var1 = row.record[76]; state.stage_var2 = row.record[77];
        state.stage_relation = u16::from_le_bytes([row.record[78], row.record[79]]);
        state.damage_dword_0x5e_94 = i32::from_le_bytes(row.record[98..102].try_into().unwrap());
        arena.economy.source_rng.insert(slot, row.entity_rng);
    } else if arena.economy.live_arena_kind(slot).is_none() {
        arena.entities[usize::from(slot)] = row.clone();
    } else {
        return Ok(false);
    }
    Ok(true)
}

fn consume_root_damage(
    arena: &SourceArena,
    actor: &mut EntitySnapshot,
    context: crate::native_world::source_target::Context<'_>,
) -> std::result::Result<u8, String> {
    let slot = actor.index;
    let mut attacker = u16::from_le_bytes([actor.record[102], actor.record[103]]);
    let mut hit = attacker != 0;
    if hit {
        let damage = i32::from_le_bytes(actor.record[98..102].try_into().unwrap());
        #[cfg(test)]
        let trace = arena.handle(slot).map(|target| {
            let mailbox = (damage, attacker);
            let writer = crate::native_world::area_damage::current_actor_mailbox_writer(target, mailbox);
            (target, mailbox, writer, actor.life)
        });
        actor.set_life(actor.life.wrapping_sub(damage));
        actor.record[102..104].fill(0);
        #[cfg(test)]
        if let Some((target, mailbox, writer, life_before)) = trace {
            let _ = crate::native_world::area_damage::take_player_mailbox_writer(target, mailbox);
            crate::native_world::area_damage::trace_player_damage_consumption(
                crate::native_world::area_damage::PlayerDamageConsumptionReceipt {
                    target, mailbox, writer, life_before, life_after: actor.life,
                    event_cursor: arena.economy.events().len(),
                },
            );
        }
    }
    let mut child = u16::from_le_bytes([actor.record[56], actor.record[57]]);
    let mut visited = 0_usize;
    while child != 0 {
        if child == slot {
            // A source self-address sees this callback's already-mutated row.
            child = u16::from_le_bytes([actor.record[56], actor.record[57]]);
            visited += 1;
            if visited > arena.entities.len() { return Err(format!("Worm {slot} child chain cycles")); }
            continue;
        }
        let current = crate::native_world::source_target::read(arena, context, child)?
            .ok_or_else(|| format!("Worm {slot} child address {child} is invalid"))?;
        if current.life < actor.life {
            #[cfg(test)]
            let life_before = actor.life;
            actor.set_life(current.life);
            attacker = current.relations.attacker.ok_or_else(|| format!("Worm child {child} lacks current attacker"))?;
            #[cfg(test)]
            super::mc2::record_inherited_damage(arena, slot, child, life_before, current.life, attacker);
            hit = true;
            break;
        }
        child = current.relations.next.ok_or_else(|| format!("Worm child {child} lacks current next link"))?;
        visited += 1;
        if visited > arena.entities.len() { return Err(format!("Worm {slot} child chain cycles")); }
    }
    actor.record[42..44].copy_from_slice(&attacker.to_le_bytes());
    if actor.life < 0 {
        actor.record[40..42].copy_from_slice(&attacker.to_le_bytes());
        Ok(2)
    } else { Ok(u8::from(hit)) }
}

/// Source action24, for both imported and typed current roots. Frozen list
/// membership supplies the scan order; each reached address is read current.
pub(crate) fn step_action24_slot(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
    peers: &[u16],
    context: crate::native_world::source_target::Context<'_>,
) -> std::result::Result<(), String> {
    let handle = arena.handle(slot).ok_or_else(|| format!("Worm {slot} lost its current handle"))?;
    let mut actor = current_root_packet(arena, slot).ok_or_else(|| format!("Worm {slot} lost its current root"))?;
    if actor.action != 24 { return Err(format!("Worm {slot} is not in action24")); }
    let hit = consume_root_damage(arena, &mut actor, context)?;
    if hit == 2 {
        actor.set_source_action(28);
    } else if hit == 1 {
        let attacker = [actor.record[42], actor.record[43]];
        actor.record[154..156].copy_from_slice(&attacker);
        actor.set_source_action(26);
    } else if actor.phase % MODEL3_DESCRIPTOR_CADENCE == 0 {
        let mut best = None;
        let mut nearest = u32::MAX;
        for &peer in peers {
            if peer == slot { continue; }
            let current = crate::native_world::source_target::read(arena, context, peer)?
                .ok_or_else(|| format!("Worm peer address {peer} is invalid"))?;
            let link = crate::native_world::source_target::read_word32(arena, context, peer)?
                .ok_or_else(|| format!("Worm peer {peer} lacks current word32"))?;
            if link != 0 { continue; }
            let distance = model3_distance_squared_2d(actor.position, current.position);
            if distance <= MODEL3_TARGET_RANGE * MODEL3_TARGET_RANGE
                && angle_distance(i32::from(actor.yaw), heading_for_positions(current.position, actor.position)) < MODEL3_TARGET_HALF_ANGLE
                && distance < nearest
            {
                best = Some(peer); nearest = distance;
            }
        }
        if let Some(peer) = best {
            actor.record[54..56].copy_from_slice(&peer.to_le_bytes());
            actor.set_source_action(27);
        }
    }
    super::imported::settle_stage_actor(&mut actor, terrain);
    if !commit_root_packet(arena, handle, &actor)? { return Err(format!("Worm {slot} changed during action24")); }
    crate::world::effects::sync_position(arena, terrain, slot, actor.position, true, false)?;
    Ok(())
}

/// Source action25: movement and RNG occur before the frozen-list scans.
pub(crate) fn step_action25_slot(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
    players: &[u16],
    peers: &[u16],
    context: crate::native_world::source_target::Context<'_>,
) -> std::result::Result<(), String> {
    let handle = arena.handle(slot).ok_or_else(|| format!("Worm {slot} lost its current handle"))?;
    let mut actor = current_root_packet(arena, slot).ok_or_else(|| format!("Worm {slot} lost its current root"))?;
    if actor.action != 25 { return Err(format!("Worm {slot} is not in action25")); }
    match consume_root_damage(arena, &mut actor, context)? {
        2 => actor.set_source_action(28),
        1 => {
            let attacker = [actor.record[42], actor.record[43]];
            actor.record[154..156].copy_from_slice(&attacker);
            actor.set_source_action(26);
            super::imported::settle_stage_actor(&mut actor, terrain);
        }
        _ => {
            if super::imported::move_stage_actor(&mut actor, terrain) {
                arena.economy.decrement_source_creature_population();
            }
            if actor.phase % MODEL3_DESCRIPTOR_CADENCE == 0 {
                let first = level2_next_rng16(actor.entity_rng);
                let second = level2_next_rng16(first);
                actor.set_entity_rng(second);
                let direction = 2 * (i32::from(first % 157) / 79) - 1;
                let roll = (i32::from(actor.source_roll()) + direction * (i32::from(second & 255) + 85)) & 2047;
                actor.set_source_roll(roll as i16);
                if actor.record[61] != 0 {
                    let mut target = None;
                    let mut nearest = u32::MAX;
                    for &candidate in players {
                        let (position, flags) = if candidate == slot {
                            (actor.position, actor.control_flags)
                        } else {
                            let current = crate::native_world::source_target::read(arena, context, candidate)?
                                .ok_or_else(|| format!("Worm target address {candidate} is invalid"))?;
                            (current.position, current.control_flags)
                        };
                        if flags & 0x20 != 0 { continue; }
                        let distance = model3_distance_squared_2d(actor.position, position);
                        if distance <= MODEL3_TARGET_RANGE * MODEL3_TARGET_RANGE
                            && angle_distance(i32::from(actor.yaw), heading_for_positions(actor.position, position)) < MODEL3_TARGET_HALF_ANGLE
                            && distance < nearest
                        { target = Some(candidate); nearest = distance; }
                    }
                    if let Some(target) = target {
                        actor.record[154..156].copy_from_slice(&target.to_le_bytes());
                        actor.set_source_action(26);
                    } else {
                        let mut selected = None;
                        let mut nearest = u32::MAX;
                        for &peer in peers {
                            if peer == slot { continue; }
                            let current = crate::native_world::source_target::read(arena, context, peer)?
                                .ok_or_else(|| format!("Worm peer address {peer} is invalid"))?;
                            let link = crate::native_world::source_target::read_word32(arena, context, peer)?
                                .ok_or_else(|| format!("Worm peer {peer} lacks current word32"))?;
                            if link != 0 { continue; }
                            let distance = model3_distance_squared_2d(actor.position, current.position);
                            if distance <= MODEL3_TARGET_RANGE * MODEL3_TARGET_RANGE
                                && angle_distance(i32::from(actor.yaw), heading_for_positions(actor.position, current.position)) < MODEL3_TARGET_HALF_ANGLE
                                && distance < nearest
                            { selected = Some(peer); nearest = distance; }
                        }
                        if let Some(peer) = selected {
                            actor.record[54..56].copy_from_slice(&peer.to_le_bytes());
                            actor.set_source_action(27);
                        }
                    }
                }
            }
        }
    }
    if !commit_root_packet(arena, handle, &actor)? { return Err(format!("Worm {slot} changed during action25")); }
    crate::world::effects::sync_position(arena, terrain, slot, actor.position, true, false)?;
    Ok(())
}

/// Source1C310 returns a shot request only after its actual movement, current
/// target check and cadence/range gate. Source1CC20 owns the later NewEvent.
pub(crate) fn step_action26_slot(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
    peers: &[u16],
    context: crate::native_world::source_target::Context<'_>,
) -> std::result::Result<Option<(u16, u16)>, String> {
    let handle = arena.handle(slot).ok_or_else(|| format!("Worm {slot} lost its current handle"))?;
    let mut actor = current_root_packet(arena, slot).ok_or_else(|| format!("Worm {slot} lost its current root"))?;
    if actor.action != 26 {
        return Err(format!("Worm {slot} is not in action26"));
    }
    let mut shot = None;
    match consume_root_damage(arena, &mut actor, context)? {
        2 => actor.set_source_action(28),
        1 => {
            let attacker = [actor.record[42], actor.record[43]];
            actor.record[154..156].copy_from_slice(&attacker);
            super::imported::settle_stage_actor(&mut actor, terrain);
        }
        _ => {
            if super::imported::move_stage_actor(&mut actor, terrain) {
                arena.economy.decrement_source_creature_population();
            }
            let target_slot = actor.source_word_96();
            // sub1ED30's Alliance tail executes after movement even when the
            // original target is the sentinel; the generic branch is inert.
            let mut allowed = true;
            if actor.record[77] == 14 {
                let remaining = u16::from_le_bytes([actor.record[50], actor.record[51]]);
                let duration = u16::from_le_bytes([actor.record[52], actor.record[53]]);
                let bits = crate::native_world::level2_class5::firefly::alliance_blink_bits(
                    remaining, duration, actor.phase, false,
                );
                actor.set_control_flags((actor.control_flags & !0x0600_0000) | bits);
                let child = u16::from_le_bytes([actor.record[56], actor.record[57]]);
                crate::native_world::level2_class5::firefly::alliance_blink_chain(arena, child, bits, context)?;
                let remaining = (remaining as i16).wrapping_sub(1);
                actor.record[50..52].copy_from_slice(&remaining.to_le_bytes());
                if remaining <= 0 {
                    allowed = false;
                } else {
                    let parent = u16::from_le_bytes([actor.record[44], actor.record[45]]);
                    if parent != 0 {
                        if parent == target_slot {
                            allowed = false;
                        } else {
                            let (selected, attacker) = if parent == slot {
                                (actor.source_word_96(), u16::from_le_bytes([actor.record[42], actor.record[43]]))
                            } else {
                                let current = crate::native_world::source_target::read(arena, context, parent)?
                                    .ok_or_else(|| format!("Worm parent address {parent} is invalid"))?;
                                let selected = current.relations.target
                                    .ok_or_else(|| format!("Worm parent {parent} lacks current target"))?;
                                let attacker = if selected == 0 {
                                    current.relations.attacker.ok_or_else(|| format!("Worm parent {parent} lacks current attacker"))?
                                } else { 0 };
                                (selected, attacker)
                            };
                            let selected = if selected == 0 { attacker } else { selected };
                            if selected != 0 && selected != target_slot { allowed = false; }
                        }
                    }
                }
            }
            let target = if !allowed || target_slot == 0 { None }
                else if target_slot == slot { Some((actor.position, actor.life, actor.control_flags)) }
                else {
                    let target = crate::native_world::source_target::read(arena, context, target_slot)?
                        .ok_or_else(|| format!("Worm target address {target_slot} is invalid"))?;
                    Some((target.position, target.life, target.control_flags))
                };
            if let Some((position, _, _)) = target.filter(|(_, life, flags)| *life >= 0 && *flags & 0x400 == 0) {
                if actor.phase & 3 == 0 {
                    actor.set_source_roll(heading_for_positions(actor.position, position) as i16);
                    for &peer in peers {
                        let (id, position) = if peer == slot { (actor.source_id, actor.position) }
                        else {
                            let current = crate::native_world::source_target::read(arena, context, peer)?
                                .ok_or_else(|| format!("Worm peer address {peer} is invalid"))?;
                            (current.source_id, current.position)
                        };
                        if id != actor.source_id
                            && (i32::from(actor.position.x) - i32::from(position.x)).abs() < i32::from(actor.array_pitch())
                            && (i32::from(actor.position.y) - i32::from(position.y)).abs() < i32::from(actor.array_pitch())
                        {
                            actor.set_source_roll(heading_for_positions(position, actor.position) as i16);
                            break;
                        }
                    }
                }
                if actor.phase % MODEL3_DESCRIPTOR_CADENCE == 0 {
                    if crate::legacy_sqrt(level2_wrapping_source_distance_3d(actor.position, position)) >= MODEL3_TARGET_RANGE {
                        actor.set_source_action(25);
                    } else {
                        shot = Some((slot, target_slot));
                    }
                }
            } else {
                actor.set_source_action(25);
            }
        }
    }
    if !commit_root_packet(arena, handle, &actor)? { return Err(format!("Worm {slot} changed during action26")); }
    crate::world::effects::sync_position(arena, terrain, slot, actor.position, true, false)?;
    Ok(shot)
}

/// Source action27 uses a direct current peer address and frozen model3 scan.
pub(crate) fn step_action27_slot(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
    peers: &[u16],
    context: crate::native_world::source_target::Context<'_>,
) -> std::result::Result<(), String> {
    let handle = arena.handle(slot).ok_or_else(|| format!("Worm {slot} lost its current handle"))?;
    let mut actor = current_root_packet(arena, slot).ok_or_else(|| format!("Worm {slot} lost its current root"))?;
    if actor.action != 27 { return Err(format!("Worm {slot} is not in action27")); }
    let peer_slot = u16::from_le_bytes([actor.record[54], actor.record[55]]);
    if peer_slot == 0 {
        actor.set_source_action(25);
    } else {
        let peer = if peer_slot == slot { actor.clone() } else {
            crate::native_world::source_target::status_parent(arena, context, peer_slot)?
                .ok_or_else(|| format!("Worm peer address {peer_slot} is invalid"))?
        };
        let valid = peer.life >= 0 && !peer.drawing_disabled() && (peer.class, peer.model) == (5, 3);
        let hit = consume_root_damage(arena, &mut actor, context)?;
        if hit != 0 {
            let attacker = u16::from_le_bytes([actor.record[42], actor.record[43]]);
            if valid && peer_slot != slot {
                if !crate::native_world::source_target::commit_worm_peer_attack(arena, context, peer_slot, attacker)? {
                    return Err(format!("Worm peer {peer_slot} changed during action27"));
                }
            }
            actor.record[54..56].fill(0);
            if hit == 2 {
                actor.set_source_action(28);
            } else {
                actor.record[154..156].copy_from_slice(&attacker.to_le_bytes());
                actor.set_source_action(26);
                super::imported::settle_stage_actor(&mut actor, terrain);
            }
        } else {
            if super::imported::move_stage_actor(&mut actor, terrain) {
                arena.economy.decrement_source_creature_population();
            }
            if !valid {
                actor.record[54..56].fill(0);
                actor.set_source_action(25);
            } else if actor.phase % MODEL3_DESCRIPTOR_CADENCE == 0 {
                let peer = if peer_slot == slot { actor.clone() } else { peer };
                match peer.action.wrapping_sub(24) {
                    0 | 1 | 3 => {
                        let leader = if peer.action == 27 {
                            if peer_slot == slot { u16::from_le_bytes([actor.record[54], actor.record[55]]) }
                            else { crate::native_world::source_target::read_word32(arena, context, peer_slot)?
                                .ok_or_else(|| format!("Worm peer {peer_slot} lacks current word32"))? }
                        } else { peer_slot };
                        if peer.action == 27 { actor.record[54..56].copy_from_slice(&leader.to_le_bytes()); }
                        let position = if leader == slot { actor.position }
                        else if leader == 0 { arena.entities[0].position }
                        else { crate::native_world::source_target::read(arena, context, leader)?
                            .ok_or_else(|| format!("Worm leader address {leader} is invalid"))?.position };
                        actor.set_source_roll(heading_for_positions(actor.position, position) as i16);
                        for &candidate in peers {
                            let (id, position) = if candidate == slot { (actor.source_id, actor.position) }
                            else {
                                let current = crate::native_world::source_target::read(arena, context, candidate)?
                                    .ok_or_else(|| format!("Worm peer address {candidate} is invalid"))?;
                                (current.source_id, current.position)
                            };
                            let dx = (i32::from(actor.position.x as i16) - i32::from(position.x as i16)).abs();
                            let dy = (i32::from(actor.position.y as i16) - i32::from(position.y as i16)).abs();
                            if id != actor.source_id && dx < 256 && dy < 256 {
                                actor.set_source_roll(heading_for_positions(position, actor.position) as i16);
                                break;
                            }
                        }
                        let maximum = i16::from_le_bytes([peer.record[138], peer.record[139]]);
                        actor.set_actual_speed(maximum.wrapping_add(peer.actual_speed));
                    }
                    2 | 6 => {
                        let target = if peer_slot == slot { actor.source_word_96() } else {
                            crate::native_world::source_target::read(arena, context, peer_slot)?
                                .and_then(|current| current.relations.target)
                                .ok_or_else(|| format!("Worm peer {peer_slot} lacks current target"))?
                        };
                        actor.record[154..156].copy_from_slice(&target.to_le_bytes());
                        actor.record[54..56].fill(0);
                        actor.set_source_action(if peer.action == 26 { 26 } else { 30 });
                    }
                    _ => { actor.record[54..56].fill(0); actor.set_source_action(25); }
                }
            }
        }
    }
    if !commit_root_packet(arena, handle, &actor)? { return Err(format!("Worm {slot} changed during action27")); }
    crate::world::effects::sync_position(arena, terrain, slot, actor.position, true, false)?;
    Ok(())
}

/// Source1C890 first converts the current linked addresses, taking the last
/// nonzero child killer, then the caller evaluates the current killer for A4.
pub(crate) fn step_action28_slot(
    arena: &mut SourceArena,
    slot: u16,
    context: crate::native_world::source_target::Context<'_>,
) -> std::result::Result<(u16, u16), String> {
    let handle = arena.handle(slot).ok_or_else(|| format!("Worm {slot} lost its current handle"))?;
    let mut actor = current_root_packet(arena, slot).ok_or_else(|| format!("Worm {slot} lost its current root"))?;
    if actor.action != 28 { return Err(format!("Worm {slot} is not in action28")); }
    let mut killer = u16::from_le_bytes([actor.record[40], actor.record[41]]);
    let mut child = u16::from_le_bytes([actor.record[56], actor.record[57]]);
    let mut visited = 0_usize;
    while child != 0 {
        let (child_killer, next) = if child == slot {
            actor.set_source_action(29);
            (u16::from_le_bytes([actor.record[40], actor.record[41]]),
             u16::from_le_bytes([actor.record[56], actor.record[57]]))
        } else {
            if !crate::native_world::source_target::commit_worm_action(arena, context, child, 29)? {
                return Err(format!("Worm {slot} child address {child} is invalid"));
            }
            let killer = crate::native_world::source_target::read_word24(arena, context, child)?
                .ok_or_else(|| format!("Worm child {child} lacks current killer"))?;
            let next = crate::native_world::source_target::read(arena, context, child)?
                .and_then(|current| current.relations.next)
                .ok_or_else(|| format!("Worm child {child} lacks current next link"))?;
            (killer, next)
        };
        if child_killer != 0 { killer = child_killer; }
        child = next;
        visited += 1;
        if visited > arena.entities.len() { return Err(format!("Worm {slot} child chain cycles")); }
    }
    actor.record[40..42].copy_from_slice(&killer.to_le_bytes());
    actor.set_source_action(29);
    if !commit_root_packet(arena, handle, &actor)? { return Err(format!("Worm {slot} changed during action28")); }
    Ok((actor.source_id, killer))
}

pub(crate) fn apply_alliance_takeover(arena: &mut SourceArena, child: SourceEntityHandle, row: &EntitySnapshot, parent: Option<SourceEntityHandle>) -> std::result::Result<bool, String> {
    let slot = child.source_slot;
    if row.index != slot || (row.class, row.model) != (5, 3) || row.record[76] != 0 || row.record[77] != 14
        || arena.handle(slot) != Some(child) || alliance_projection(arena, slot).is_none_or(|current| current.source_id != row.source_id) {
        return Ok(false);
    }
    let parent_slot = u16::from_le_bytes([row.record[44], row.record[45]]);
    let source = arena.economy.authored_class5.iter_mut().find(|row| row.active && row.source_entity_index == slot && row.model == 3)
        .ok_or_else(|| format!("Alliance Worm {slot} lost its typed root"))?;
    source.action = row.action; source.word_0x96_150 = row.source_word_96();
    let state = arena.economy.authored_class5_model3.iter_mut().find(|row| row.source_entity_index == slot)
        .ok_or_else(|| format!("Alliance Worm {slot} lost its state"))?;
    state.parent_entity_index = parent_slot;
    state.stage_remaining = i16::from_le_bytes([row.record[50], row.record[51]]);
    state.alliance_duration = u16::from_le_bytes([row.record[52], row.record[53]]);
    state.stage_var1 = row.record[76]; state.stage_var2 = row.record[77];
    state.stage_relation = u16::from_le_bytes([row.record[78], row.record[79]]);
    if let Some(parent) = parent.filter(|parent| parent.source_slot == parent_slot) {
        if !arena.bind_creature_parent(child, parent) { return Err(format!("Alliance Worm {slot} lost its parent binding")); }
    }
    Ok(true)
}

pub(crate) fn step_alliance_stage_slot(
    arena: &mut SourceArena, terrain: &mut SessionTerrain, slot: u16,
    peers: &[u16],
    context: crate::native_world::source_target::Context<'_>,
) -> std::result::Result<Vec<(u16, u8)>, String> {
    let source_index = arena.economy.authored_class5.iter().position(|row| row.active && row.source_entity_index == slot && row.model == 3 && row.action == 31)
        .ok_or_else(|| format!("Alliance Worm {slot} lost its stage root"))?;
    let state_index = arena.economy.authored_class5_model3.iter().position(|row| row.source_entity_index == slot)
        .ok_or_else(|| format!("Alliance Worm {slot} lost its state"))?;
    let mut source = arena.economy.authored_class5[source_index];
    let mut state = arena.economy.authored_class5_model3[state_index];
    if state.stage_var2 != 14 { return Err(format!("Worm {slot} is not allied")); }
    state.stage_remaining = state.stage_remaining.wrapping_sub(1);
    let parent = if state.stage_remaining > 0 {
        crate::native_world::source_target::read(arena, context, state.parent_entity_index)?.filter(|row| row.life > 0)
    } else { None };
    let bits = crate::native_world::level2_class5::firefly::alliance_blink_bits(
        state.stage_remaining as u16, state.alliance_duration as u16, source.source_byte_3e, parent.is_none(),
    );
    state.control_flags = (state.control_flags & !0x0600_0000) | bits;
    crate::native_world::level2_class5::firefly::alliance_blink_chain(arena, state.word_0x34, bits, context)?;
    let mut feedback = Vec::new();
    if let Some(parent) = parent {
        let previous = source.word_0x96_150;
        source.word_0x96_150 = state.parent_entity_index;
        match arena.economy.model3_source_prepass(&mut source, &mut state) {
            2 => state.stage_remaining = 1,
            hit => {
                arena.economy.model3_step_source_movement(&mut source, &mut state, terrain);
                if hit == 1 {
                    let attacker = crate::native_world::source_target::read(arena, context, source.word_0x26)?;
                    if source.word_0x26 != state.parent_entity_index && attacker.is_none_or(|row| (row.class, row.model) != (5, 3)) {
                        source.word_0x96_150 = source.word_0x26;
                        source.action = 26;
                        feedback.push((state.parent_entity_index, 19));
                    }
                } else if source.source_byte_3e & 7 == 0 {
                    if state.control_flags & MODEL3_RAW_MOTION_COLLISION_BIT == 0 {
                        source.roll = heading_for_positions(source.position, parent.position) as i16;
                        if source.source_byte_3e & 63 == 0 {
                            source.entity_rng = level2_next_rng16(source.entity_rng); let first = source.entity_rng;
                            source.entity_rng = level2_next_rng16(source.entity_rng);
                            let delta = (2 * (i32::from(first % 157) / 79) - 1) * (i32::from(source.entity_rng & 255) + 85);
                            source.roll = ((i32::from(source.roll) + delta) & 2047) as i16;
                        }
                    }
                    alliance_retarget_nearby(arena, &mut source, state.entity_id, state.render_pitch, peers, context)?;
                }
            }
        }
        let target = parent.relations.target.unwrap_or(0);
        let selected = if target != 0 { target } else { parent.relations.attacker.unwrap_or(0) };
        source.word_0x96_150 = if selected == 0 { previous } else { selected };
        if let Some(target) = crate::native_world::source_target::read(arena, context, source.word_0x96_150)? {
            if (target.relations.parent == Some(state.parent_entity_index) && target.relations.stage_mode == Some(14))
                || target.life <= 0 || target.drawing_disabled() {
                source.word_0x96_150 = 0;
            } else if crate::legacy_sqrt(level2_wrapping_source_distance_3d(source.position, target.position)) < MODEL3_TARGET_RANGE {
                source.action = 26;
                feedback.push((state.parent_entity_index, 24));
            }
        }
    } else {
        state.stage_var2 = 10; state.stage_var1 = state.stage_relation as u8;
        state.parent_entity_index = 0; source.word_0x96_150 = 0;
    }
    arena.economy.source_rng.insert(slot, source.entity_rng);
    arena.economy.authored_class5[source_index] = source;
    arena.economy.authored_class5_model3[state_index] = state;
    crate::world::effects::sync_position(arena, terrain, slot, source.position, source.active, false)?;
    Ok(feedback)
}

pub(crate) fn step_alliance_attack_slot(
    arena: &mut SourceArena, terrain: &mut SessionTerrain, slot: u16,
    peers: &[u16],
    context: crate::native_world::source_target::Context<'_>,
) -> std::result::Result<(), String> {
    let source_index = arena.economy.authored_class5.iter().position(|row| row.active && row.source_entity_index == slot && row.model == 3 && row.action == 26)
        .ok_or_else(|| format!("Alliance Worm {slot} lost its attack root"))?;
    let state_index = arena.economy.authored_class5_model3.iter().position(|row| row.source_entity_index == slot)
        .ok_or_else(|| format!("Alliance Worm {slot} lost its state"))?;
    let mut source = arena.economy.authored_class5[source_index];
    let mut state = arena.economy.authored_class5_model3[state_index];
    if state.stage_var2 != 14 { return Err(format!("Worm {slot} is not allied")); }
    match arena.economy.model3_source_prepass(&mut source, &mut state) {
        2 => source.action = 28,
        1 => {
            source.word_0x96_150 = source.word_0x26;
            arena.economy.model3_source_terrain_settle(&mut source, terrain);
        }
        _ => {
            arena.economy.model3_step_source_movement(&mut source, &mut state, terrain);
            let bits = crate::native_world::level2_class5::firefly::alliance_blink_bits(
                state.stage_remaining as u16, state.alliance_duration as u16, source.source_byte_3e, false,
            );
            state.control_flags = (state.control_flags & !0x0600_0000) | bits;
            crate::native_world::level2_class5::firefly::alliance_blink_chain(arena, state.word_0x34, bits, context)?;
            state.stage_remaining = state.stage_remaining.wrapping_sub(1);
            let parent = crate::native_world::source_target::read(arena, context, state.parent_entity_index)?;
            let selected = parent.map(|parent| {
                let target = parent.relations.target.unwrap_or(0);
                if target == 0 { parent.relations.attacker.unwrap_or(0) } else { target }
            }).unwrap_or(0);
            let allowed = state.stage_remaining > 0 && (parent.is_none() ||
                (source.word_0x96_150 != state.parent_entity_index && (selected == 0 || selected == source.word_0x96_150)));
            let target = if allowed { crate::native_world::source_target::read(arena, context, source.word_0x96_150)? } else { None };
            if let Some(target) = target.filter(|target| target.life >= 0 && !target.drawing_disabled()) {
                if source.source_byte_3e & 3 == 0 {
                    source.roll = heading_for_positions(source.position, target.position) as i16;
                    alliance_retarget_nearby(arena, &mut source, state.entity_id, state.render_pitch, peers, context)?;
                }
                if source.source_byte_3e % MODEL3_DESCRIPTOR_CADENCE == 0 {
                    if crate::legacy_sqrt(level2_wrapping_source_distance_3d(source.position, target.position)) >= MODEL3_TARGET_RANGE {
                        source.action = 25;
                    } else {
                        let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(source.position, target.position);
                        let mut launch_position = source.position; launch_position.z = launch_position.z.wrapping_add(source.fov);
                        let spawned = arena.economy.spawn_fireball_projectile_with_payload(
                            level2_dynamic_economy::Level2FireballProjectileLaunchPayload {
                                origin: FireballProjectileOrigin::Worm, position: launch_position, yaw, pitch,
                                axis_position: target.position, source_id_entity_index: state.entity_id,
                                holder_entity_index: 0, source_subspell_index: 500, source_mana: 0, source_dword_0x10: 0,
                                target_source_entity_index: target.slot, target_class: target.class, target_model: target.model,
                            }, 0,
                        );
                        if spawned.is_some() {
                            Level2EconomyState::apply_creature_fireball_target_feedback(&mut arena.entities, target.slot);
                            arena.economy.events.push(Level2EconomyEvent::AuthoredClass5Model3Sound { source_entity_index: slot, sound_id: 8 });
                        }
                    }
                }
            } else { source.action = 25; }
        }
    }
    arena.economy.source_rng.insert(slot, source.entity_rng);
    arena.economy.authored_class5[source_index] = source;
    arena.economy.authored_class5_model3[state_index] = state;
    crate::world::effects::sync_position(arena, terrain, slot, source.position, source.active, false)?;
    Ok(())
}

fn alliance_retarget_nearby(
    arena: &SourceArena,
    source: &mut Level2AuthoredClass5Source,
    source_id: u16,
    extent: i16,
    peers: &[u16],
    context: crate::native_world::source_target::Context<'_>,
) -> std::result::Result<(), String> {
    for &slot in peers {
        let Some(peer) = crate::native_world::source_target::read(arena, context, slot)? else { continue };
        if peer.source_id != source_id
            && i32::from(peer.position.x).abs_diff(i32::from(source.position.x)) < extent.max(0) as u32
            && i32::from(peer.position.y).abs_diff(i32::from(source.position.y)) < extent.max(0) as u32
        {
            source.roll = heading_for_positions(peer.position, source.position) as i16;
            break;
        }
    }
    Ok(())
}

/// Apply sub_12500 to the typed Worm root's actual route state before its
/// source visit, including the next pass after Alliance restores stage10.
pub(crate) fn prepare_alliance_stage_pass(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    slots: &[u16],
    context: crate::native_world::source_target::Context<'_>,
) -> std::result::Result<(), String> {
    for &slot in slots {
        let Some(source_index) = arena.economy.authored_class5.iter().position(|row|
            row.active && row.source_entity_index == slot && row.model == 3) else { continue };
        let Some(state_index) = arena.economy.authored_class5_model3.iter().position(|row|
            row.source_entity_index == slot) else { continue };
        let mut source = arena.economy.authored_class5[source_index];
        let mut state = arena.economy.authored_class5_model3[state_index];
        let mut index = state.stage_var1 as i8;
        let mut mode = state.stage_var2 as i8;
        let mut error = None;
        let mut route = super::super::stage_route::Route {
            model: 3, position: source.position, action: &mut source.action,
            index: &mut index, mode: &mut mode, relation: &mut state.stage_relation,
        };
        route.prepare(runtime, |target| match crate::native_world::source_target::read(arena, context, target) {
            Ok(target) => target.is_some_and(|target| target.life < 0 || target.drawing_disabled()),
            Err(problem) => { error = Some(problem); false }
        });
        if let Some(problem) = error { return Err(problem); }
        state.stage_var1 = index as u8;
        state.stage_var2 = mode as u8;
        arena.economy.authored_class5[source_index] = source;
        arena.economy.authored_class5_model3[state_index] = state;
    }
    Ok(())
}

/// Source4B6F0 allocates the root and then each body through NewEvent.
/// Particle geometry is validated before the first allocation, and a failed
/// later allocation retains the already-published prefix.
pub(crate) fn construct_authored(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    authored_index: u16,
    mut position: Position,
    max_life: i32,
    particles: &[ParticleParameter],
) -> Option<Level2AuthoredClass5Source> {
    if max_life <= 0 {
        return None;
    }
    let geometry = model3_geometry(particles)?;
    let slot = arena.allocate_source_slot_with_terrain(terrain)?;
    let rng = level2_next_rng16(arena.economy.new_event_rng(slot));
    let phase = arena.economy.class5_model3_constructor_phase;
    arena.economy.class5_model3_constructor_phase = phase.wrapping_add(1);
    let yaw = i16::try_from(rng & 0x07ff).expect("11-bit Worm yaw") - 1;
    position.z = terrain.ground_height(position);
    let properties = crate::level_definition::WORM_PROPERTIES;
    let mut source = Level2AuthoredClass5Source {
        source_entity_index: slot,
        entity_rng: rng,
        authored_index,
        model: 3,
        action: 25,
        minimum_speed: properties.minimum_speed,
        maximum_speed: properties.maximum_speed,
        actual_speed: properties.initial_speed,
        max_life: properties.max_life,
        life: properties.max_life,
        mana: properties.initial_mana(),
        max_mana: properties.max_mana(),
        subspell_index: 0,
        dword_0x10: i32::from(slot % 100),
        pending_word_0x62: 0,
        word_0x26: 0,
        word_0x24: 0,
        byte_0x3b: 0,
        word_0x96_150: 0,
        byte_0x46_70: 0,
        particle_index: 88,
        source_byte_3e: phase,
        source_byte_39: MODEL3_DESCRIPTOR_CADENCE
            .wrapping_sub(phase % MODEL3_DESCRIPTOR_CADENCE)
            .wrapping_add(4),
        yaw,
        pitch: yaw,
        roll: yaw,
        fov: 0,
        shift_rot_x: 0,
        shift_rot_y: 0,
        position,
        phase: 0,
        drawing_enabled: true,
        active: true,
    };
    arena.economy.source_owner.insert(slot, 0);
    arena.economy.source_rng.insert(slot, rng);
    arena.economy.authored_class5.push(source);
    let mut previous_slot = 0;
    let mut first_slot = 0;
    for child_index in 0..MODEL3_CHILD_COUNT {
        let Some(child_slot) = arena.allocate_source_slot_with_terrain(terrain) else {
            break;
        };
        let _new_event_rng = arena.economy.new_event_rng(child_slot);
        let child =
            initialize_model3_child(&source, &geometry, child_index, child_slot, previous_slot);
        if previous_slot == 0 {
            first_slot = child_slot;
        } else if let Some(previous) = arena
            .economy
            .authored_class5_model3_children
            .iter_mut()
            .find(|child| child.source_entity_index == previous_slot)
        {
            previous.word_0x34 = child_slot;
        }
        arena
            .economy
            .source_rng
            .insert(child_slot, source.entity_rng);
        arena.economy.authored_class5_model3_children.push(child);
        crate::world::effects::sync_position(arena, terrain, child_slot, position, true, false)
            .ok()?;
        previous_slot = child_slot;
    }
    arena
        .economy
        .authored_class5_model3
        .push(initialize_model3_state(&source, &geometry, first_slot));
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false).ok()?;
    // The authored override follows the constructor and does not rewrite the
    // already-copied body max-life values.
    source.max_life = max_life;
    source.life = max_life;
    let row = arena
        .economy
        .authored_class5
        .iter_mut()
        .find(|row| row.source_entity_index == slot)?;
    *row = source;
    Some(source)
}

fn initialize_model3_child(
    source: &Level2AuthoredClass5Source,
    geometry: &Model3Geometry,
    child_index: usize,
    child_slot: u16,
    previous_slot: u16,
) -> Level2AuthoredClass5Model3Child {
    let entity_rng = source.entity_rng;
    let part = geometry.children[child_index];
    Level2AuthoredClass5Model3Child {
        source_entity_index: child_slot,
        root_authored_index: source.authored_index,
        entity_rng,
        player_owner_entity_index: 0,
        parent_entity_index: source.source_entity_index,
        // qmemcpy copies the root's word 0x32; the constructor then
        // writes the previous linked entity into every new child.
        word_0x32: if previous_slot == 0 {
            source.source_entity_index
        } else {
            previous_slot
        },
        word_0x34: 0,
        word_0x36: part.spacing,
        word_0x24: 0,
        pending_word_0x62: 0,
        word_0x26: 0,
        word_0x96: 0,
        damage_dword_0x5e_94: 0,
        dword_0x10: source.dword_0x10,
        minimum_speed: source.minimum_speed,
        maximum_speed: source.maximum_speed,
        actual_speed: source.actual_speed,
        max_life: source.max_life,
        life: source.life,
        mana: source.max_mana / 32,
        control_flags: 8,
        model: source.model,
        action: 232,
        source_byte_3e: u8::try_from(child_index).expect("model-3 child index fits"),
        source_byte_39: source.source_byte_39,
        byte_0x3b: 0,
        byte_0x43: 10,
        byte_0x44: 0,
        byte_0x46: 0,
        word_0x2c: 0,
        particle_index: 89 + u16::try_from(child_index).expect("Worm child selector fits"),
        yaw: source.yaw,
        pitch: source.pitch,
        roll: source.roll,
        fov: source.fov,
        render_yaw: part.yaw,
        render_pitch: part.axis,
        render_roll: part.axis,
        render_fov: part.fov,
        shift_rot_x: 0,
        shift_rot_y: 0,
        position: source.position,
        phase: 0,
        drawing_enabled: true,
        active: true,
    }
}

fn initialize_model3_state(
    source: &Level2AuthoredClass5Source,
    geometry: &Model3Geometry,
    first_slot: u16,
) -> Level2AuthoredClass5Model3State {
    Level2AuthoredClass5Model3State {
        source_entity_index: source.source_entity_index,
        entity_id: source.source_entity_index,
        stage_var1: 0,
        stage_var2: 0,
        stage_relation: 0,
        parent_entity_index: 0,
        stage_remaining: 0,
        alliance_duration: 0,
        damage_dword_0x5e_94: 0,
        word_0x32: 0,
        word_0x34: first_slot,
        word_0x36: 96,
        control_flags: 8,
        byte_0x43: 10,
        byte_0x44: 0,
        word_0x2c: 0,
        descriptor_index: MODEL3_DESCRIPTOR_INDEX,
        last_child_root_phase: u8::MAX,
        render_yaw: geometry.root_yaw,
        render_pitch: geometry.root_axis,
        render_roll: geometry.root_axis,
        render_fov: geometry.root_fov,
    }
}

impl Level2EconomyState {
    #[inline]
    fn model3_state_index(&self, source_slot: u16) -> Option<usize> {
        self.authored_class5_model3
            .iter()
            .position(|state| state.source_entity_index == source_slot)
    }

    fn model3_child_slots(&self, first_slot: u16) -> Vec<u16> {
        let mut slots = Vec::with_capacity(MODEL3_CHILD_COUNT);
        let mut next = first_slot;
        while next != 0 && slots.len() < MODEL3_CHILD_COUNT {
            slots.push(next);
            next = self
                .authored_class5_model3_children
                .iter()
                .find(|child| child.source_entity_index == next)
                .map_or(0, |child| child.word_0x34);
        }
        slots
    }

    /// Advance one model-3 root through the reachable source callbacks. The
    /// action-232 children are presentation rows copied by the constructor;
    /// no model-27 child callback is attached to them.
    pub(in crate::native_world) fn step_authored_class5_model3(
        &mut self,
        source: &mut Level2AuthoredClass5Source,
        target: Option<EntitySnapshot>,
        terrain: &SessionTerrain,
    ) {
        let Some(state_index) = self.model3_state_index(source.source_entity_index) else {
            return;
        };
        let mut model3 = self.authored_class5_model3[state_index];
        match source.action {
            24 => self.step_model3_action24(source, &mut model3, terrain),
            25 => self.step_model3_action25(source, &mut model3, target, terrain),
            26 => self.step_model3_action26(source, &mut model3, target, terrain),
            27 => self.step_model3_action27(source, &mut model3, terrain),
            28 => self.step_model3_action28(source, &model3),
            29 => self.step_model3_action29(source, &mut model3, terrain),
            // 216..220 are model-27 actions owned by sub_4D000. A malformed
            // model-3 row is intentionally inert rather than borrowing that
            // carrier's state machine.
            _ => {}
        }
        self.authored_class5_model3[state_index] = model3;
        self.source_rng
            .insert(source.source_entity_index, source.entity_rng);
    }

    /// Source action 24 calls sub_1BD90: damage and peer selection are
    /// followed by terrain settling, with no movement probe.
    fn step_model3_action24(
        &mut self,
        source: &mut Level2AuthoredClass5Source,
        model3: &mut Level2AuthoredClass5Model3State,
        terrain: &SessionTerrain,
    ) {
        match self.model3_source_prepass(source, model3) {
            2 => source.action = 28,
            1 => {
                source.word_0x96_150 = source.word_0x26;
                source.action = if MODEL3_DESCRIPTOR_FLAGS & 8 != 0 { 30 } else { 26 };
            }
            _ if MODEL3_DESCRIPTOR_FLAGS & 4 == 0
                && source.source_byte_3e % MODEL3_DESCRIPTOR_CADENCE == 0 =>
            {
                if let Some(peer) = self.model3_select_root_peer(source) {
                    model3.word_0x32 = peer;
                    source.action = 27;
                }
            }
            _ => {}
        }
        self.model3_source_terrain_settle(source, terrain);
    }

    fn model3_source_prepass(
        &self,
        source: &mut Level2AuthoredClass5Source,
        model3: &mut Level2AuthoredClass5Model3State,
    ) -> u8 {
        let mut state = 0_u8;
        if source.pending_word_0x62 != 0 {
            source.life = source.life.wrapping_sub(model3.damage_dword_0x5e_94);
            source.word_0x26 = source.pending_word_0x62;
            source.pending_word_0x62 = 0;
            // sub_1BF90 reads dword 0x5e94 but does not clear it. Reset and
            // replay therefore retain the same mailbox/hash value.
            state = 1;
        } else {
            source.word_0x26 = 0;
        }

        for child_slot in self.model3_child_slots(model3.word_0x34) {
            let Some(child) = self
                .authored_class5_model3_children
                .iter()
                .find(|child| child.active && child.source_entity_index == child_slot)
            else {
                continue;
            };
            if child.life < source.life {
                source.life = child.life;
                source.word_0x26 = child.word_0x26;
                state = 1;
                break;
            }
        }

        if source.life < 0 {
            source.word_0x24 = source.word_0x26;
            state = 2;
        }
        state
    }

    fn step_model3_action25(
        &mut self,
        source: &mut Level2AuthoredClass5Source,
        model3: &mut Level2AuthoredClass5Model3State,
        target: Option<EntitySnapshot>,
        terrain: &SessionTerrain,
    ) {
        let state = self.model3_source_prepass(source, model3);
        if state == 2 {
            source.action = 28;
            return;
        }
        if state == 1 {
            source.word_0x96_150 = source.word_0x26;
            source.action = if MODEL3_DESCRIPTOR_FLAGS & 8 != 0 {
                30
            } else {
                26
            };
            self.model3_source_terrain_settle(source, terrain);
            return;
        }

        // sub_1B8C0 runs before the descriptor cadence gate.
        let _ = self.model3_step_source_movement(source, model3, terrain);
        if source.source_byte_3e % MODEL3_DESCRIPTOR_CADENCE != 0 {
            return;
        }
        let first = level2_next_rng16(source.entity_rng);
        let second = level2_next_rng16(first);
        source.entity_rng = second;
        let direction = 2 * (i32::from(first % 0x009d) / 79) - 1;
        let delta = (i32::from(second & 0x00ff) + 85) * direction;
        source.roll = ((i32::from(source.roll) + delta) & 0x07ff) as i16;

        if source.source_byte_39 == 0 {
            return;
        }
        let target = target
            .filter(|target| {
                target.active
                    && target.class == 3
                    && target.life >= 0
                    && target.control_flags & MODEL3_RAW_SOURCE_DISABLED_BIT == 0
            })
            .filter(|target| {
                model3_distance_squared_2d(source.position, target.position)
                    <= MODEL3_TARGET_RANGE * MODEL3_TARGET_RANGE
                    && angle_distance(
                        i32::from(source.yaw),
                        heading_for_positions(source.position, target.position),
                    ) < MODEL3_TARGET_HALF_ANGLE
            });
        if let Some(target) = target {
            source.word_0x96_150 = target.index;
            source.action = if MODEL3_DESCRIPTOR_FLAGS & 8 != 0 {
                30
            } else {
                26
            };
        } else if MODEL3_DESCRIPTOR_FLAGS & 4 == 0 {
            if let Some(peer) = self.model3_select_root_peer(source) {
                model3.word_0x32 = peer;
                source.action = 27;
            }
        }
    }

    /// Port `sub_1F990 -> sub_1C310`. The native-world caller supplies the
    /// lossless target snapshot and applies the established player-feedback
    /// carrier when the sound-8 hit event is emitted.
    fn step_model3_action26(
        &mut self,
        source: &mut Level2AuthoredClass5Source,
        model3: &mut Level2AuthoredClass5Model3State,
        target: Option<EntitySnapshot>,
        terrain: &SessionTerrain,
    ) {
        match self.model3_source_prepass(source, model3) {
            2 => {
                source.action = 28;
                return;
            }
            1 => {
                source.word_0x96_150 = source.word_0x26;
                source.action = if MODEL3_DESCRIPTOR_FLAGS & 8 != 0 {
                    30
                } else {
                    26
                };
                self.model3_source_terrain_settle(source, terrain);
                return;
            }
            _ => {}
        }

        let _ = self.model3_step_source_movement(source, model3, terrain);
        let Some(target) = target.filter(|target| {
            target.active
                && target.life >= 0
                && target.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
        }) else {
            source.action = 25;
            return;
        };
        if crate::legacy_sqrt(level2_wrapping_source_distance_3d(
            source.position,
            target.position,
        )) >= MODEL3_TARGET_RANGE
        {
            source.action = 25;
            return;
        }
        if source.source_byte_3e & 3 == 0 {
            source.roll = i16::try_from(heading_for_positions(source.position, target.position))
                .expect("model-3 target heading fits");
            self.model3_retarget_heading_nearby(source, MODEL3_ACTION26_ARRAY_PITCH);
        }
        if source.source_byte_3e % MODEL3_DESCRIPTOR_CADENCE != 0 {
            return;
        }

        let (yaw, pitch) =
            crate::level1_mission::projectile_guidance_angles(source.position, target.position);
        let mut launch_position = source.position;
        launch_position.z = launch_position.z.wrapping_add(source.fov);
        let spawned = self.spawn_fireball_projectile_with_payload(
            level2_dynamic_economy::Level2FireballProjectileLaunchPayload {
                origin: FireballProjectileOrigin::Worm,
                position: launch_position,
                yaw,
                pitch,
                axis_position: target.position,
                source_id_entity_index: source.source_entity_index,
                holder_entity_index: 0,
                source_subspell_index: 500,
                source_mana: 0,
                source_dword_0x10: 0,
                target_source_entity_index: target.index,
                target_class: 0,
                target_model: 0,
            },
            0,
        );
        if spawned.is_some() {
            self.events
                .push(Level2EconomyEvent::AuthoredClass5Model3Sound {
                    source_entity_index: source.source_entity_index,
                    sound_id: 8,
                });
        }
    }

    /// Port `sub_1F9E0 -> sub_1C560`, using the model-3 root sidecar for the
    /// source word 0x32 relation and retaining source-order peer ties.
    fn step_model3_action27(
        &mut self,
        source: &mut Level2AuthoredClass5Source,
        model3: &mut Level2AuthoredClass5Model3State,
        terrain: &SessionTerrain,
    ) {
        if model3.word_0x32 == 0 {
            source.action = 25;
            return;
        }
        let peer = self.model3_peer_info(model3.word_0x32);
        let peer_valid = peer.is_some_and(|peer| {
            peer.life >= 0
                && peer.drawing_enabled
                && peer.source_entity_index != source.source_entity_index
        });
        match self.model3_source_prepass(source, model3) {
            2 => {
                if let Some(peer) = peer.filter(|_| peer_valid) {
                    self.model3_update_peer_state(peer.source_entity_index, source.word_0x26);
                }
                model3.word_0x32 = 0;
                source.action = 28;
                return;
            }
            1 => {
                if let Some(peer) = peer.filter(|_| peer_valid) {
                    self.model3_update_peer_state(peer.source_entity_index, source.word_0x26);
                }
                model3.word_0x32 = 0;
                source.word_0x96_150 = source.word_0x26;
                source.action = if MODEL3_DESCRIPTOR_FLAGS & 8 != 0 {
                    30
                } else {
                    26
                };
                self.model3_source_terrain_settle(source, terrain);
                return;
            }
            _ => {}
        }
        let _ = self.model3_step_source_movement(source, model3, terrain);
        let Some(peer) = peer.filter(|_| peer_valid) else {
            model3.word_0x32 = 0;
            source.action = 25;
            return;
        };
        if source.source_byte_3e % MODEL3_DESCRIPTOR_CADENCE != 0 {
            return;
        }

        match peer.action.wrapping_sub(24) {
            0 | 1 => {
                source.roll = i16::try_from(heading_for_positions(source.position, peer.position))
                    .expect("model-3 peer heading fits");
                self.model3_retarget_heading_nearby(source, 256);
                source.actual_speed = peer.maximum_speed.wrapping_add(peer.actual_speed);
            }
            2 => {
                source.word_0x96_150 = peer.word_0x96;
                model3.word_0x32 = 0;
                source.action = 26;
            }
            3 => {
                model3.word_0x32 = peer.word_0x32;
                let heading_target = self
                    .model3_position_for_slot(peer.word_0x32)
                    .expect("the source arena retains the sentinel and live peer rows");
                source.roll = i16::try_from(heading_for_positions(source.position, heading_target))
                    .expect("model-3 chained peer heading fits");
                self.model3_retarget_heading_nearby(source, 256);
                source.actual_speed = peer.maximum_speed.wrapping_add(peer.actual_speed);
            }
            6 => {
                source.word_0x96_150 = peer.word_0x96;
                model3.word_0x32 = 0;
                source.action = 30;
            }
            _ => {
                model3.word_0x32 = 0;
                source.action = 25;
            }
        }
    }

    fn step_model3_action28(
        &mut self,
        source: &mut Level2AuthoredClass5Source,
        model3: &Level2AuthoredClass5Model3State,
    ) {
        let mut transferred = source.word_0x24;
        for child_slot in self.model3_child_slots(model3.word_0x34) {
            if let Some(child) = self
                .authored_class5_model3_children
                .iter_mut()
                .find(|child| child.active && child.source_entity_index == child_slot)
            {
                child.action = 29;
                if child.word_0x24 != 0 {
                    transferred = child.word_0x24;
                }
            }
        }
        source.word_0x24 = transferred;
        source.action = 29;
    }

    fn step_model3_action29(
        &mut self,
        source: &mut Level2AuthoredClass5Source,
        model3: &mut Level2AuthoredClass5Model3State,
        terrain: &SessionTerrain,
    ) {
        if source.source_byte_3e & 7 != 0 {
            return;
        }
        if source.mana > 0 {
            source.entity_rng = level2_next_rng16(source.entity_rng);
            let player_owner = self.source_owner(source.source_entity_index);
            let _ = self.spawn_mana_sphere_from_death(
                source.source_entity_index,
                source.position,
                source.yaw,
                source.entity_rng,
                source.mana,
                player_owner,
                terrain,
            );
            self.source_owner.insert(source.source_entity_index, 0);
        }
        if model3.control_flags & 0x0010_0000 == 0 {
            let _ = self.spawn_death_burst_from_death_with_id(
                source.source_entity_index,
                model3.entity_id,
                source.position,
            );
        }
        source.drawing_enabled = false;
        model3.control_flags |= ENTITY_DRAW_DISABLED_BIT;
    }

    fn model3_step_source_movement(
        &mut self,
        source: &mut Level2AuthoredClass5Source,
        model3: &mut Level2AuthoredClass5Model3State,
        terrain: &SessionTerrain,
    ) -> u8 {
        if model3.control_flags & 0x800 != 0 {
            model3.control_flags &= !0x800;
            return 4;
        }
        let before = source.position;
        let mut predicted = before;
        model3_source_settle_position(&mut predicted, terrain);
        crate::move_entity(&mut predicted, source.yaw, 0, source.actual_speed);
        terrain.set_source_predicted_position(predicted);
        let crossed_tile = before.x >> 8 != predicted.x >> 8 || before.y >> 8 != predicted.y >> 8;
        if !crossed_tile {
            source.position = predicted;
            model3.control_flags &= !MODEL3_RAW_MOTION_COLLISION_BIT;
            source.yaw = model3_heading_after_turn(source.yaw, source.roll);
            return 1;
        }

        if !model3_candidate_blocked(model3, predicted, source.yaw, terrain) {
            source.position = predicted;
            model3.control_flags &= !MODEL3_RAW_MOTION_COLLISION_BIT;
            source.yaw = model3_heading_after_turn(source.yaw, source.roll);
            return 2;
        }

        model3.control_flags |= MODEL3_RAW_MOTION_COLLISION_BIT;
        let original_yaw = source.yaw;
        for heading in model3_source_retry_headings(original_yaw) {
            source.yaw = heading;
            let mut candidate = before;
            model3_source_settle_position(&mut candidate, terrain);
            crate::move_entity(&mut candidate, heading, 0, source.actual_speed);
            terrain.set_source_predicted_position(candidate);
            if !model3_candidate_blocked(model3, candidate, heading, terrain) {
                source.position = candidate;
                source.yaw = model3_heading_after_turn(heading, source.roll);
                return 3;
            }
        }

        if MODEL3_DESCRIPTOR_FLAGS & 1 != 0 {
            source.life = -1;
            self.decrement_source_creature_population();
        }
        4
    }

    fn model3_source_terrain_settle(
        &self,
        source: &mut Level2AuthoredClass5Source,
        terrain: &SessionTerrain,
    ) {
        model3_source_settle_position(&mut source.position, terrain);
    }

    fn model3_position_for_slot(&self, source_slot: u16) -> Option<Position> {
        if source_slot == 0 {
            // Source entity zero is the permanently zeroed sentinel row.
            return Some(Position::default());
        }
        self.authored_class5
            .iter()
            .find(|source| {
                source.active && source.model == 3 && source.source_entity_index == source_slot
            })
            .map(|source| source.position)
            .or_else(|| {
                self.authored_class5_model3_children
                    .iter()
                    .find(|child| {
                        child.active && child.model == 3 && child.source_entity_index == source_slot
                    })
                    .map(|child| child.position)
            })
    }

    fn model3_peer_info(&self, source_slot: u16) -> Option<Model3Peer> {
        let source = self.authored_class5.iter().find(|source| {
            source.active && source.model == 3 && source.source_entity_index == source_slot
        })?;
        let state = self
            .authored_class5_model3
            .iter()
            .find(|state| state.source_entity_index == source_slot)?;
        Some(Model3Peer {
            source_entity_index: source.source_entity_index,
            position: source.position,
            life: source.life,
            maximum_speed: source.maximum_speed,
            actual_speed: source.actual_speed,
            action: source.action,
            word_0x96: source.word_0x96_150,
            word_0x32: state.word_0x32,
            drawing_enabled: source.drawing_enabled,
        })
    }

    fn model3_same_model_positions(&self, source_slot: u16) -> Vec<(u16, Position)> {
        let mut positions = self
            .authored_class5
            .iter()
            .filter(|source| source.active && source.model == 3)
            .map(|source| (source.source_entity_index, source.position))
            .collect::<Vec<_>>();
        positions.sort_unstable_by_key(|(slot, _)| *slot);
        positions.retain(|(slot, _)| *slot != source_slot);
        positions
    }

    fn model3_retarget_heading_nearby(
        &self,
        source: &mut Level2AuthoredClass5Source,
        proximity: i16,
    ) {
        for (_, position) in self.model3_same_model_positions(source.source_entity_index) {
            let dx = i32::from(position.x.wrapping_sub(source.position.x).cast_signed()).abs();
            let dy = i32::from(position.y.wrapping_sub(source.position.y).cast_signed()).abs();
            if dx < i32::from(proximity) && dy < i32::from(proximity) {
                source.roll = i16::try_from(heading_for_positions(position, source.position))
                    .expect("model-3 nearby heading fits");
                break;
            }
        }
    }

    fn model3_select_root_peer(&self, source: &Level2AuthoredClass5Source) -> Option<u16> {
        let mut nearest: Option<(u16, u32)> = None;
        let mut peers = self
            .authored_class5
            .iter()
            .filter(|peer| peer.active && peer.model == source.model)
            .collect::<Vec<_>>();
        peers.sort_unstable_by_key(|peer| peer.source_entity_index);
        for peer in peers {
            let Some(state) = self
                .authored_class5_model3
                .iter()
                .find(|state| state.source_entity_index == peer.source_entity_index)
            else {
                continue;
            };
            if peer.source_entity_index == source.source_entity_index || state.word_0x32 != 0 {
                continue;
            }
            let distance = model3_distance_squared_2d(source.position, peer.position);
            if distance > MODEL3_TARGET_RANGE * MODEL3_TARGET_RANGE
                || angle_distance(
                    i32::from(source.yaw),
                    heading_for_positions(peer.position, source.position),
                ) >= MODEL3_TARGET_HALF_ANGLE
            {
                continue;
            }
            if nearest.is_none_or(|(_, nearest_distance)| distance < nearest_distance) {
                nearest = Some((peer.source_entity_index, distance));
            }
        }
        nearest.map(|(slot, _)| slot)
    }

    fn model3_update_peer_state(&mut self, peer_slot: u16, relation: u16) {
        if let Some(peer) = self
            .authored_class5
            .iter_mut()
            .find(|peer| peer.source_entity_index == peer_slot)
        {
            peer.word_0x96_150 = relation;
            peer.action = if MODEL3_DESCRIPTOR_FLAGS & 8 != 0 {
                30
            } else {
                26
            };
        }
        if let Some(state) = self
            .authored_class5_model3
            .iter_mut()
            .find(|state| state.source_entity_index == peer_slot)
        {
            state.word_0x32 = 0;
        }
    }

    /// Run the source action-232 predecessor follower or the action-29
    /// phase-gated death conversion for one typed Worm body row.
    pub(crate) fn step_authored_class5_model3_child_slot(
        &mut self,
        source_slot: u16,
        predecessor_fallback: Option<EntitySnapshot>,
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(child_index) = self
            .authored_class5_model3_children
            .iter()
            .position(|child| {
                child.active && child.source_entity_index == source_slot && child.model == 3
            })
        else {
            return false;
        };
        let mut child = self.authored_class5_model3_children[child_index];
        match child.action {
            232 => {
                let predecessor = self
                    .authored_class5
                    .iter()
                    .find(|source| source.active && source.source_entity_index == child.word_0x32)
                    .map(|source| (5, source.position, source.yaw))
                    .or_else(|| {
                        self.authored_class5_model3_children
                            .iter()
                            .find(|source| {
                                source.active && source.source_entity_index == child.word_0x32
                            })
                            .map(|source| (5, source.position, source.yaw))
                    })
                    .or_else(|| {
                        predecessor_fallback
                            .map(|source| (source.class, source.position, source.yaw))
                    });
                let Some((predecessor_class, predecessor_position, predecessor_yaw)) = predecessor
                else {
                    return false;
                };
                if predecessor_class != 5 {
                    child.drawing_enabled = false;
                    child.control_flags |= ENTITY_DRAW_DISABLED_BIT;
                }
                if child.source_byte_39 != 0 {
                    let yaw =
                        i16::try_from(heading_for_positions(child.position, predecessor_position))
                            .expect("Worm body heading fits i16");
                    let horizontal = crate::legacy_sqrt(model3_distance_squared_2d(
                        child.position,
                        predecessor_position,
                    ));
                    let pitch = crate::pitch_for_delta(
                        i32::try_from(horizontal).expect("wrapped Worm distance fits i32"),
                        i32::from(predecessor_position.z.wrapping_sub(child.position.z)),
                    );
                    let mut position = predecessor_position;
                    crate::move_entity(&mut position, yaw, pitch, child.word_0x36.wrapping_neg());
                    terrain.set_source_predicted_position(position);
                    child.position = position;
                    child.yaw = yaw;
                    child.pitch = pitch;
                    if child.pending_word_0x62 != 0 {
                        child.life = child.life.wrapping_sub(child.damage_dword_0x5e_94);
                        child.word_0x26 = child.pending_word_0x62;
                        child.pending_word_0x62 = 0;
                    } else {
                        child.word_0x26 = 0;
                    }
                } else if child.source_byte_3e & 3 == 0 {
                    child.position = predecessor_position;
                    child.yaw = predecessor_yaw;
                }
            }
            29 => {
                if child.source_byte_3e & 7 == 0 {
                    if child.mana > 0 {
                        child.entity_rng = level2_next_rng16(child.entity_rng);
                        let _ = self.spawn_mana_sphere_from_death(
                            source_slot,
                            child.position,
                            child.yaw,
                            child.entity_rng,
                            child.mana,
                            child.player_owner_entity_index,
                            terrain,
                        );
                        child.player_owner_entity_index = 0;
                    }
                    if child.control_flags & 0x0010_0000 == 0 {
                        let _ = self.spawn_death_burst_from_death_with_id(
                            source_slot,
                            child.parent_entity_index,
                            child.position,
                        );
                    }
                    child.drawing_enabled = false;
                    child.control_flags |= ENTITY_DRAW_DISABLED_BIT;
                }
            }
            _ => return false,
        }
        self.source_rng.insert(source_slot, child.entity_rng);
        self.authored_class5_model3_children[child_index] = child;
        if child.active {
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: true,
                    x: child.position.x,
                    y: child.position.y,
                })
                .is_ok()
        } else {
            true
        }
    }

    /// Dispatch the model-3 family through the same source cursor bookkeeping
    /// used by the shared class-5 entry point. This bridge is needed because
    /// the legacy class-5 module only selects action 25 at its older seam.
    pub(crate) fn step_authored_class5_model3_slot_with_target(
        &mut self,
        source_slot: u16,
        target: Option<EntitySnapshot>,
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(source_index) = self
            .authored_class5
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut source = self.authored_class5[source_index];
        if source.model != 3 {
            return false;
        }
        self.step_authored_class5_model3(&mut source, target, terrain);
        source.source_byte_3e = source.source_byte_3e.wrapping_add(1);
        source.phase = source.phase.wrapping_add(1);
        self.authored_class5[source_index] = source;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }
}

#[cfg(test)]
mod source_motion_tests {
    use super::*;

    #[test]
    fn constructor_reads_live_particle_geometry_and_preflights_before_allocation() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.canonical_terrain().clone();
        let mut particles = session.session.definition.presentation.particles.clone();
        particles[88].speed = 1000; particles[88].rotation_speed = 200;
        particles[89].speed = 333; particles[89].rotation_speed = 777;
        let mut changed = crate::level_definition::WORM_PROPERTIES;
        changed.body_scale_percent = 50;
        let changed_geometry = model3_geometry_with_properties(&particles, changed).unwrap();
        assert_eq!((changed_geometry.children[0].axis, changed_geometry.children[0].fov), (166, 388),
            "the real geometry rule consumes the supplied typed scale");
        let before = (arena.economy.free_slot_count(), arena.economy.source_list_epoch(),
            arena.economy.authored_class5_model3.len());
        assert!(construct_authored(&mut arena, &mut terrain, 238, session.state.position, 9000,
            &particles[..104]).is_none());
        assert_eq!((arena.economy.free_slot_count(), arena.economy.source_list_epoch(),
            arena.economy.authored_class5_model3.len()), before);
        let root = construct_authored(&mut arena, &mut terrain, 238, session.state.position, 9000,
            &particles).unwrap();
        let state = arena.economy.authored_class5_model3.iter()
            .find(|row| row.source_entity_index == root.source_entity_index).unwrap();
        assert_eq!((state.render_yaw, state.render_pitch, state.render_roll, state.render_fov),
            (100, 600, 600, 120));
        let child = arena.economy.authored_class5_model3_children.iter()
            .find(|row| row.source_entity_index == state.word_0x34).unwrap();
        assert_eq!((child.particle_index, child.render_yaw, child.render_pitch,
            child.render_roll, child.render_fov, child.word_0x36), (89, 388, 216, 216, 505, 270));
    }

    #[test]
    fn movement_preserves_collision_flag_and_publishes_only_real_proposals() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.canonical_terrain().clone();
        let position = Position { x: 0x4080, y: 0x5080, z: 0 };
        let mut source = construct_authored(&mut arena, &mut terrain, 238, position, 9000, &session.session.definition.presentation.particles).unwrap();
        let mut state = *arena.economy.authored_class5_model3.iter()
            .find(|state| state.source_entity_index == source.source_entity_index).unwrap();
        source.position.z = terrain.ground_height(source.position);
        source.yaw = 0;
        source.roll = 200;
        source.actual_speed = 30;
        state.control_flags = 8;
        let mut expected = source.position;
        crate::move_entity(&mut expected, 0, 0, 30);
        let old_scratch = Position { x: 1, y: 2, z: 3 };
        terrain.set_source_predicted_position(old_scratch);
        assert_eq!(arena.economy.model3_step_source_movement(&mut source, &mut state, &terrain), 1);
        assert_eq!((source.position, source.yaw), (expected, 5));
        assert_eq!(state.control_flags & 8, 8, "map collision remains enabled");
        assert_eq!(terrain.source_predicted_position(), expected);

        let before = source.position;
        state.control_flags |= 0x800;
        terrain.set_source_predicted_position(old_scratch);
        assert_eq!(arena.economy.model3_step_source_movement(&mut source, &mut state, &terrain), 4);
        assert_eq!(source.position, before);
        assert_eq!(state.control_flags & 0x808, 8);
        assert_eq!(terrain.source_predicted_position(), old_scratch);
    }

    #[test]
    fn stationary_callback_consumes_damage_and_settles_without_moving() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.canonical_terrain().clone();
        let position = Position { x: 0x4080, y: 0x5080, z: 0 };
        let mut source = construct_authored(&mut arena, &mut terrain, 238, position, 9000, &session.session.definition.presentation.particles).unwrap();
        let mut state = *arena.economy.authored_class5_model3.iter()
            .find(|state| state.source_entity_index == source.source_entity_index).unwrap();
        let ground = terrain.ground_height(position);
        source.position.z = ground.wrapping_add(128);
        source.life = 100;
        source.action = 24;
        source.pending_word_0x62 = 2;
        state.damage_dword_0x5e_94 = 7;
        let old_scratch = Position { x: 1, y: 2, z: 3 };
        terrain.set_source_predicted_position(old_scratch);
        arena.economy.step_model3_action24(&mut source, &mut state, &terrain);
        assert_eq!((source.life, source.action, source.word_0x96_150), (93, 26, 2));
        assert_eq!(source.position, Position { z: ground.wrapping_add(64), ..position });
        assert_eq!(terrain.source_predicted_position(), old_scratch);
    }

    #[test]
    fn scheduled_stationary_peer_scan_reads_current_worm_word32() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.canonical_terrain().clone();
        let here = Position { x: 0x4080, y: 0x5080, z: 0 };
        let there = Position { x: 0x4180, ..here };
        let first = construct_authored(&mut arena, &mut terrain, 238, here, 9000, &session.session.definition.presentation.particles).unwrap();
        let peer = construct_authored(&mut arena, &mut terrain, 239, there, 9000, &session.session.definition.presentation.particles).unwrap();
        let root = arena.economy.authored_class5.iter_mut()
            .find(|source| source.source_entity_index == first.source_entity_index).unwrap();
        root.action = 24;
        root.source_byte_3e = 0;
        root.yaw = heading_for_positions(there, here) as i16;
        // The dormant raw packet is not the current typed Worm's peer link.
        arena.entities[usize::from(peer.source_entity_index)].record[54..56]
            .copy_from_slice(&7_u16.to_le_bytes());
        step_action24_slot(&mut arena, &mut terrain, first.source_entity_index,
            &[first.source_entity_index, peer.source_entity_index], Default::default()).unwrap();
        let root = arena.economy.authored_class5.iter()
            .find(|source| source.source_entity_index == first.source_entity_index).unwrap();
        let state = arena.economy.authored_class5_model3.iter()
            .find(|state| state.source_entity_index == first.source_entity_index).unwrap();
        assert_eq!((root.action, state.word_0x32), (27, peer.source_entity_index));
        assert_eq!(u16::from_le_bytes(arena.entities[usize::from(peer.source_entity_index)].record[54..56]
            .try_into().unwrap()), 7);
    }

    #[test]
    fn scheduled_roam_settles_once_and_draws_rng_when_scan_is_disabled() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.canonical_terrain().clone();
        let position = Position { x: 0x4080, y: 0x5080, z: 0 };
        let first = construct_authored(&mut arena, &mut terrain, 238, position, 9000, &session.session.definition.presentation.particles).unwrap();
        let ground = terrain.ground_height(position);
        let root = arena.economy.authored_class5.iter_mut()
            .find(|source| source.source_entity_index == first.source_entity_index).unwrap();
        root.action = 25; root.source_byte_3e = 0; root.source_byte_39 = 0;
        root.yaw = 0; root.roll = 0; root.actual_speed = 30;
        root.position.z = ground.wrapping_add(128);
        let initial_rng = root.entity_rng;
        let expected_rng = level2_next_rng16(level2_next_rng16(initial_rng));
        step_action25_slot(&mut arena, &mut terrain, first.source_entity_index,
            &[], &[], Default::default()).unwrap();
        let root = arena.economy.authored_class5.iter()
            .find(|source| source.source_entity_index == first.source_entity_index).unwrap();
        assert_eq!(root.action, 25);
        assert_eq!(root.entity_rng, expected_rng);
        assert_eq!(root.position.z, ground.wrapping_add(64), "movement performs the only settle");
        assert_eq!(terrain.source_predicted_position(), root.position);
    }

    #[test]
    fn scheduled_peer_follow_uses_current_speed_and_zero_link_returns_before_damage() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.canonical_terrain().clone();
        let here = Position { x: 0x4080, y: 0x5080, z: 0 };
        let there = Position { x: 0x4280, ..here };
        let first = construct_authored(&mut arena, &mut terrain, 238, here, 9000, &session.session.definition.presentation.particles).unwrap();
        let peer = construct_authored(&mut arena, &mut terrain, 239, there, 9000, &session.session.definition.presentation.particles).unwrap();
        let slot = first.source_entity_index;
        let root = arena.economy.authored_class5.iter_mut().find(|row| row.source_entity_index == slot).unwrap();
        root.action = 27; root.pending_word_0x62 = 2; root.life = 100;
        arena.economy.authored_class5_model3.iter_mut().find(|row| row.source_entity_index == slot).unwrap()
            .damage_dword_0x5e_94 = 7;
        step_action27_slot(&mut arena, &mut terrain, slot, &[], Default::default()).unwrap();
        let root = arena.economy.authored_class5.iter_mut().find(|row| row.source_entity_index == slot).unwrap();
        assert_eq!((root.action, root.life, root.pending_word_0x62), (25, 100, 2));
        root.action = 27; root.pending_word_0x62 = 0; root.source_byte_3e = 0;
        let state = arena.economy.authored_class5_model3.iter_mut().find(|row| row.source_entity_index == slot).unwrap();
        state.word_0x32 = peer.source_entity_index; state.control_flags |= 0x800;
        let current_peer = arena.economy.authored_class5.iter_mut()
            .find(|row| row.source_entity_index == peer.source_entity_index).unwrap();
        current_peer.action = 25; current_peer.maximum_speed = 90; current_peer.actual_speed = 30;
        arena.entities[usize::from(peer.source_entity_index)].record[138..140]
            .copy_from_slice(&1_i16.to_le_bytes());
        step_action27_slot(&mut arena, &mut terrain, slot, &[slot, peer.source_entity_index], Default::default()).unwrap();
        let root = arena.economy.authored_class5.iter().find(|row| row.source_entity_index == slot).unwrap();
        assert_eq!((root.action, root.actual_speed), (27, 120));

        let root = arena.economy.authored_class5.iter_mut().find(|row| row.source_entity_index == slot).unwrap();
        root.pending_word_0x62 = 2;
        let state = arena.economy.authored_class5_model3.iter_mut().find(|row| row.source_entity_index == slot).unwrap();
        state.damage_dword_0x5e_94 = 7;
        let peer_state = arena.economy.authored_class5_model3.iter_mut()
            .find(|row| row.source_entity_index == peer.source_entity_index).unwrap();
        peer_state.word_0x32 = 91;
        step_action27_slot(&mut arena, &mut terrain, slot, &[], Default::default()).unwrap();
        let root = arena.economy.authored_class5.iter().find(|row| row.source_entity_index == slot).unwrap();
        let target = arena.economy.authored_class5.iter()
            .find(|row| row.source_entity_index == peer.source_entity_index).unwrap();
        let peer_state = arena.economy.authored_class5_model3.iter()
            .find(|row| row.source_entity_index == peer.source_entity_index).unwrap();
        assert_eq!((root.action, root.life, root.word_0x96_150), (26, 93, 2));
        assert_eq!((target.action, target.word_0x96_150, peer_state.word_0x32), (26, 2, 0));
    }

    #[test]
    fn scheduled_attack_checks_range_only_on_source_cadence() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.canonical_terrain().clone();
        let here = Position { x: 0x4080, y: 0x5080, z: 0 };
        let first = construct_authored(&mut arena, &mut terrain, 238, here, 9000, &session.session.definition.presentation.particles).unwrap();
        let slot = first.source_entity_index;
        let target = arena.allocate_source_slot_with_terrain(&mut terrain).unwrap();
        let mut packet = crate::native_world::cave_features::new_event(target, 5,
            Position { x: here.x.wrapping_add(10_000), ..here });
        packet.class = 3; packet.model = 0; packet.record[67] = 3; packet.record[68] = 0;
        packet.set_life(1_000);
        arena.entities[usize::from(target)] = packet;
        let root = arena.economy.authored_class5.iter_mut().find(|row| row.source_entity_index == slot).unwrap();
        root.action = 26; root.source_byte_3e = 1; root.word_0x96_150 = target;
        arena.economy.authored_class5_model3.iter_mut().find(|row| row.source_entity_index == slot).unwrap()
            .control_flags |= 0x800;
        assert_eq!(step_action26_slot(&mut arena, &mut terrain, slot, &[], Default::default()).unwrap(), None);
        assert_eq!(arena.economy.authored_class5.iter().find(|row| row.source_entity_index == slot).unwrap().action, 26);
        arena.economy.authored_class5.iter_mut().find(|row| row.source_entity_index == slot).unwrap().source_byte_3e = 0;
        arena.economy.authored_class5_model3.iter_mut().find(|row| row.source_entity_index == slot).unwrap()
            .control_flags |= 0x800;
        assert_eq!(step_action26_slot(&mut arena, &mut terrain, slot, &[], Default::default()).unwrap(), None);
        assert_eq!(arena.economy.authored_class5.iter().find(|row| row.source_entity_index == slot).unwrap().action, 25);
        arena.economy.authored_class5.iter_mut().find(|row| row.source_entity_index == slot).unwrap().action = 26;
        arena.economy.authored_class5_model3.iter_mut().find(|row| row.source_entity_index == slot).unwrap()
            .control_flags |= 0x800;
        arena.entities[usize::from(target)].set_position(here);
        assert_eq!(step_action26_slot(&mut arena, &mut terrain, slot, &[], Default::default()).unwrap(), Some((slot, target)));
    }

    #[test]
    fn allied_attack_decrements_before_current_parent_target_filter() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.canonical_terrain().clone();
        let here = Position { x: 0x4080, y: 0x5080, z: 0 };
        let root = construct_authored(&mut arena, &mut terrain, 238, here, 9000, &session.session.definition.presentation.particles).unwrap();
        let slot = root.source_entity_index;
        let parent = arena.allocate_source_slot_with_terrain(&mut terrain).unwrap();
        let target = arena.allocate_source_slot_with_terrain(&mut terrain).unwrap();
        let mut parent_row = crate::native_world::cave_features::new_event(parent, 5, here);
        parent_row.class = 5; parent_row.model = 21; parent_row.record[67] = 5; parent_row.record[68] = 21;
        parent_row.record[154..156].copy_from_slice(&target.to_le_bytes());
        arena.entities[usize::from(parent)] = parent_row;
        let mut target_row = crate::native_world::cave_features::new_event(target, 5, here);
        target_row.class = 3; target_row.model = 0; target_row.record[67] = 3; target_row.record[68] = 0;
        target_row.set_life(1_000);
        arena.entities[usize::from(target)] = target_row;
        let actor = arena.economy.authored_class5.iter_mut().find(|row| row.source_entity_index == slot).unwrap();
        actor.action = 26; actor.source_byte_3e = 1; actor.word_0x96_150 = target;
        let state = arena.economy.authored_class5_model3.iter_mut().find(|row| row.source_entity_index == slot).unwrap();
        state.stage_var2 = 14; state.parent_entity_index = parent;
        state.stage_remaining = 2; state.alliance_duration = 40; state.control_flags |= 0x800;
        assert_eq!(step_action26_slot(&mut arena, &mut terrain, slot, &[], Default::default()).unwrap(), None);
        let actor = arena.economy.authored_class5.iter().find(|row| row.source_entity_index == slot).unwrap();
        let state = arena.economy.authored_class5_model3.iter().find(|row| row.source_entity_index == slot).unwrap();
        assert_eq!((actor.action, state.stage_remaining), (26, 1));
        arena.economy.authored_class5_model3.iter_mut().find(|row| row.source_entity_index == slot).unwrap()
            .control_flags |= 0x800;
        assert_eq!(step_action26_slot(&mut arena, &mut terrain, slot, &[], Default::default()).unwrap(), None);
        let actor = arena.economy.authored_class5.iter().find(|row| row.source_entity_index == slot).unwrap();
        let state = arena.economy.authored_class5_model3.iter().find(|row| row.source_entity_index == slot).unwrap();
        assert_eq!((actor.action, state.stage_remaining, actor.word_0x96_150), (25, 0, target));
    }

    #[test]
    fn prekill_uses_current_child_killer_and_updates_the_whole_chain() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.canonical_terrain().clone();
        let root = construct_authored(&mut arena, &mut terrain, 238, session.state.position, 9000,
            &session.session.definition.presentation.particles).unwrap();
        let slot = root.source_entity_index;
        let first = arena.economy.authored_class5_model3.iter()
            .find(|state| state.source_entity_index == slot).unwrap().word_0x34;
        let second = arena.economy.authored_class5_model3_children.iter()
            .find(|row| row.source_entity_index == first).unwrap().word_0x34;
        let actor = arena.economy.authored_class5.iter_mut().find(|row| row.source_entity_index == slot).unwrap();
        actor.action = 28; actor.word_0x24 = 2;
        arena.economy.authored_class5_model3_children.iter_mut()
            .find(|row| row.source_entity_index == first).unwrap().word_0x24 = 3;
        arena.economy.authored_class5_model3_children.iter_mut()
            .find(|row| row.source_entity_index == second).unwrap().word_0x24 = 4;
        arena.entities[usize::from(second)].record[40..42].copy_from_slice(&99_u16.to_le_bytes());
        assert_eq!(step_action28_slot(&mut arena, slot, Default::default()).unwrap(), (slot, 4));
        let actor = arena.economy.authored_class5.iter().find(|row| row.source_entity_index == slot).unwrap();
        assert_eq!((actor.action, actor.word_0x24), (29, 4));
        assert!(arena.economy.authored_class5_model3_children.iter()
            .filter(|row| row.parent_entity_index == slot).all(|row| row.action == 29));
        assert_eq!(u16::from_le_bytes(arena.entities[usize::from(second)].record[40..42]
            .try_into().unwrap()), 99);
    }

    #[test]
    fn zero_mana_kill_keeps_the_source_owner_on_root_and_body() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.canonical_terrain().clone();
        let mut root = construct_authored(&mut arena, &mut terrain, 238, session.state.position, 9000,
            &session.session.definition.presentation.particles).unwrap();
        let mut state = *arena.economy.authored_class5_model3.iter()
            .find(|row| row.source_entity_index == root.source_entity_index).unwrap();
        let child_slot = state.word_0x34;
        root.mana = 0; root.source_byte_3e = 0; state.control_flags |= 0x0010_0000;
        arena.economy.source_owner.insert(root.source_entity_index, 42);
        arena.economy.step_model3_action29(&mut root, &mut state, &terrain);
        assert_eq!(arena.economy.source_owner(root.source_entity_index), 42);
        assert!(!root.drawing_enabled);
        let child = arena.economy.authored_class5_model3_children.iter_mut()
            .find(|row| row.source_entity_index == child_slot).unwrap();
        child.action = 29; child.source_byte_3e = 0; child.mana = 0;
        child.player_owner_entity_index = 42; child.control_flags |= 0x0010_0000;
        assert!(arena.economy.step_authored_class5_model3_child_slot(child_slot, None, &terrain));
        let child = arena.economy.authored_class5_model3_children.iter()
            .find(|row| row.source_entity_index == child_slot).unwrap();
        assert_eq!(child.player_owner_entity_index, 42);
        assert!(!child.drawing_enabled);
    }
}
