//! Level 2 class-5/model-9 action-72 transition and action-73 movement.

use crate::{FlightWorld, Position, Tile, WATER_AMBIENCE_MASK};

use super::super::*;

impl crate::campaign::source_arena::SourceArena {
    /// Move the legacy typed Skeleton into the ordinary lossless carrier at
    /// its existing address. This is representation transfer, not a birth:
    /// allocation, RNG, intrusive links and the source generation stay put.
    pub(crate) fn promote_authored_skeleton(
        &mut self,
        slot: u16,
        particles: &[ParticleParameter],
        descriptor: &[u8],
    ) -> std::result::Result<bool, String> {
        use crate::campaign::source_arena::CreatureRootOrigin;
        let Some(source) = self.economy.authored_class5.iter()
            .find(|row| row.active && row.model == 9 && row.source_entity_index == slot).copied()
        else { return Ok(false); };
        let runtime = self.economy.authored_class5_runtime.iter()
            .find(|row| row.source_entity_index == slot).copied()
            .unwrap_or(Level2AuthoredClass5RuntimeState {
                source_entity_index: slot, byte_0x38: 1, xtype: 3,
                descriptor_index: 80, pending_damage: 0, linked_head: 0,
            });
        let handle = self.handle(slot).ok_or_else(|| format!("Skeleton {slot} lost its address"))?;
        let particle = particles.get(usize::from(source.particle_index))
            .ok_or_else(|| format!("Skeleton {slot} has no current particle"))?;
        let pose = campaign_shared_families::ParticlePose {
            animation_limit: particle.animation_limit().ok_or("Skeleton particle has no animation limit")?,
            array_yaw: (particle.rotation_speed / 2) as i16,
            array_pitch: (particle.speed / 2) as i16,
            array_roll: (particle.speed / 2) as i16,
            array_fov: (particle.rotation_speed / 2) as i16,
        };
        let scan_period = descriptor.get(26..28)
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
            .filter(|period| *period != 0).ok_or("Skeleton descriptor has no scan period")?;
        let mut raw = campaign_shared_families::construct_skeleton_source(
            slot, source.entity_rng, source.position, source.source_byte_3e, scan_period, pose,
        );
        raw.set_source_action(source.action);
        raw.set_entity_rng(source.entity_rng);
        raw.set_max_life(source.max_life);
        raw.set_life(source.life);
        raw.set_mana(source.mana);
        raw.max_mana = source.max_mana;
        raw.record[144..148].copy_from_slice(&source.max_mana.to_le_bytes());
        raw.set_actual_speed(source.actual_speed);
        raw.record[136..138].copy_from_slice(&source.minimum_speed.to_le_bytes());
        raw.record[138..140].copy_from_slice(&source.maximum_speed.to_le_bytes());
        raw.set_subspell_index(source.subspell_index);
        raw.set_source_dword_10(source.dword_0x10);
        raw.set_yaw(source.yaw);
        raw.set_pitch(source.pitch);
        raw.set_source_roll(source.roll);
        raw.set_fov(source.fov);
        raw.record[98..102].copy_from_slice(&runtime.pending_damage.to_le_bytes());
        for (offset, value) in [(40, source.word_0x24), (42, source.word_0x26),
            (56, runtime.linked_head), (102, source.pending_word_0x62), (154, source.word_0x96_150)] {
            raw.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
        raw.record[60] = runtime.byte_0x38;
        raw.record[61] = source.source_byte_39;
        raw.record[63] = source.byte_0x3b;
        raw.record[69] = runtime.xtype;
        raw.record[74] = source.byte_0x46_70;
        raw.particle_parameter_index = Some(source.particle_index);
        raw.record[94..96].copy_from_slice(&source.particle_index.to_le_bytes());
        raw.record[88..90].copy_from_slice(&source.shift_rot_x.to_le_bytes());
        raw.record[90..92].copy_from_slice(&source.shift_rot_x.to_le_bytes());
        raw.record[92..94].copy_from_slice(&source.shift_rot_y.to_le_bytes());
        raw.parameter_relation = Some(runtime.descriptor_index);
        raw.record[164..168].copy_from_slice(&(u32::from(runtime.descriptor_index) + 1).to_le_bytes());
        raw.set_map_links(self.economy.map_chains.old_map_entity(slot), self.economy.map_chains.next_entity(slot));
        raw.next_relation = self.entities[usize::from(slot)].next_relation;
        raw.record[4..8].copy_from_slice(&self.entities[usize::from(slot)].record[4..8]);
        raw.set_source_player_entity_index(*self.economy.source_owner.get(&slot).unwrap_or(&0));
        if !source.drawing_enabled { raw.disable_drawing(); }
        self.economy.authored_class5.retain(|row| row.source_entity_index != slot);
        self.economy.authored_class5_runtime.retain(|row| row.source_entity_index != slot);
        self.entities[usize::from(slot)] = raw;
        if self.handle(slot) != Some(handle) || !self.bind_raw_creature_origin(
            handle, CreatureKind::Skeleton,
            CreatureRootOrigin::AuthoredRecord { authored_record: source.authored_index },
        ) {
            return Err(format!("Skeleton {slot} changed identity during promotion"));
        }
        Ok(true)
    }

    /// Complete the ordinary 68BF0 prepass for the opening families which
    /// retain multipart projections. Other families run their own adapter.
    /// Membership is frozen before callbacks; child links read live occupants.
    pub(crate) fn step_opening_creature_proximity(
        &mut self,
        models: &[Vec<u16>; 29],
        player: Position,
    ) {
        for model in [0, 3, 9, 19] {
            for &slot in &models[model] {
                let Some(index) = self.economy.authored_class5.iter().position(|row| {
                    row.active && row.source_entity_index == slot && usize::from(row.model) == model
                }) else {
                    self.step_raw_creature_proximity(&[slot], player);
                    continue;
                };
                let (flags, mut child) = match model {
                    0 => (
                        self.economy
                            .authored_dragons
                            .iter()
                            .find(|row| row.source_entity_index == slot)
                            .map_or(0, |row| row.control_flags),
                        self.economy
                            .authored_class5_runtime
                            .iter()
                            .find(|row| row.source_entity_index == slot)
                            .map_or(0, |row| row.linked_head),
                    ),
                    3 => self
                        .economy
                        .authored_class5_model3
                        .iter()
                        .find(|row| row.source_entity_index == slot)
                        .map_or((0, 0), |row| (row.control_flags, row.word_0x34)),
                    19 => (
                        self.economy
                            .authored_fireflies
                            .iter()
                            .find(|row| row.source_entity_index == slot)
                            .map_or(0, |row| row.control_flags),
                        0,
                    ),
                    _ => (0, 0),
                };
                let root = &mut self.economy.authored_class5[index];
                let inherited = if root.life < 0 {
                    root.source_byte_39 = 250;
                    None
                } else if root.source_byte_39 != 0 {
                    let prior = root.source_byte_39;
                    root.source_byte_39 = prior.wrapping_sub(1);
                    Some(prior)
                } else if flags & 1 != 0 {
                    None
                } else {
                    // These constructors initialize byte3A to zero; none of
                    // their source callbacks writes a proximity delay.
                    let dx = i32::from(root.position.x.wrapping_sub(player.x) as i16);
                    let dy = i32::from(root.position.y.wrapping_sub(player.y) as i16);
                    let distance = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32;
                    if distance < 0x0240_0000 {
                        root.source_byte_39 = 16;
                        Some(18)
                    } else {
                        None
                    }
                };
                let Some(value) = inherited else { continue };
                for _ in 0..self.entities.len() {
                    if child == 0 {
                        break;
                    }
                    if let Some(part) = self
                        .economy
                        .authored_dragon_parts
                        .iter_mut()
                        .find(|part| part.source_entity_index == child && part.active)
                    {
                        part.source_byte_39 = value;
                        child = part.next_entity_index;
                    } else if let Some(part) = self
                        .economy
                        .authored_class5_model3_children
                        .iter_mut()
                        .find(|part| part.source_entity_index == child && part.active)
                    {
                        part.source_byte_39 = value;
                        child = part.word_0x34;
                    } else if let Some(part) = self.entities.get_mut(usize::from(child)) {
                        part.record[61] = value;
                        child = part.word_34();
                    } else {
                        break;
                    }
                }
            }
        }
    }

    /// Authored `sub_4A310 -> sub_4BBB0` uses the same lossless carrier and
    /// action family as reset-imported and converted Skeletons.
    pub(crate) fn allocate_authored_skeleton_raw(
        &mut self,
        authored_record: u16,
        mut position: Position,
        max_life: i32,
        stage_index: Option<u8>,
        terrain: &mut SessionTerrain,
        particles: &[ParticleParameter],
        descriptor: &[u8],
    ) -> std::result::Result<Option<u16>, String> {
        use crate::campaign::source_arena::CreatureRootOrigin;
        use crate::level_definition::CreatureKind;
        if max_life <= 0 || descriptor.len() < 34 {
            return Err("Skeleton requires positive life and descriptor 80".into());
        }
        let scan_period = u16::from_le_bytes([descriptor[26], descriptor[27]]);
        if scan_period == 0 {
            return Err("Skeleton descriptor scan period is zero".into());
        }
        let particle = particles
            .get(220)
            .ok_or_else(|| "Skeleton requires particle 220".to_owned())?;
        let pose = campaign_shared_families::ParticlePose {
            animation_limit: particle
                .animation_limit()
                .ok_or_else(|| "Skeleton particle 220 has invalid animation mode".to_owned())?,
            array_yaw: (particle.rotation_speed / 2) as i16,
            array_pitch: (particle.speed / 2) as i16,
            array_roll: (particle.speed / 2) as i16,
            array_fov: (particle.rotation_speed / 2) as i16,
        };
        let Some(slot) = self.allocate_source_slot_with_terrain(terrain) else {
            return Ok(None);
        };
        let seed = self.economy.new_event_rng(slot);
        let phase = self.economy.class5_model9_constructor_phase;
        self.economy.class5_model9_constructor_phase = phase.wrapping_add(1);
        position.z = terrain.ground_height(position);
        let mut row = campaign_shared_families::construct_skeleton_source(
            slot,
            seed,
            position,
            phase,
            scan_period,
            pose,
        );
        row.bind_frame_zero_max_life(max_life);
        // Skeleton defers sub_12330 until its emergence countdown finishes.
        row.record[78..80].copy_from_slice(&u16::from(stage_index.unwrap_or(0)).to_le_bytes());
        let rng = row.entity_rng;
        self.entities[usize::from(slot)] = row;
        self.economy.source_rng.insert(slot, rng);
        crate::world::effects::sync_position(self, terrain, slot, position, true, false)?;
        let material_mask =
            u32::from_le_bytes(descriptor[20..24].try_into().expect("descriptor width"));
        let clearance = i16::from_le_bytes([descriptor[12], descriptor[13]]);
        if campaign_shared_families::skeleton_placement_blocked(
            terrain,
            position,
            material_mask,
            clearance,
        ) {
            crate::world::effects::sync_position(self, terrain, slot, position, false, false)?;
            self.entities[usize::from(slot)].reclaim_source_row();
            self.economy.release_slot(slot, Some(rng));
            return Ok(None);
        }
        let handle = self
            .handle(slot)
            .ok_or_else(|| format!("Skeleton constructor lost slot {slot}"))?;
        if !self.bind_raw_creature_origin(
            handle,
            CreatureKind::Skeleton,
            CreatureRootOrigin::AuthoredRecord { authored_record },
        ) {
            return Err(format!(
                "Skeleton constructor lost identity for slot {slot}"
            ));
        }
        Ok(Some(slot))
    }
}

/// `str_D7BD6[80]`, the descriptor installed by `sub_4BBB0` for model 9.
/// The row's `dword_0x14` is `0xffff_ffff`, so the source material mask is
/// empty: model-9 movement rejects cave/terrain geometry, not a material
/// class.  The remaining values are the source's movement geometry and turn
/// contract.
const MODEL9_SLOPE_LIMIT: i32 = 0x0046;
const MODEL9_CAVE_CLEARANCE: i32 = 0x0200;
const MODEL9_PARTICLE_FOV: i32 = 125;
const MODEL9_TURN_LIMIT: i16 = 0x0016;
const MODEL9_YAW_MASK: i32 = 0x07ff;
const MODEL9_ACTION_COUNTDOWN: u8 = 72;
const MODEL9_ACTION_MOVEMENT: u8 = 73;
const MODEL9_MOVEMENT_COUNTDOWN: i32 = 400;
const MODEL9_MOVEMENT_PARTICLE: u16 = 201;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Model9MoveResult {
    Accepted { heading: i16, position: Position },
    Blocked { heading: i16 },
}

impl Level2EconomyState {
    #[inline]
    pub(super) fn step_authored_class5_model9(
        &self,
        source: &mut Level2AuthoredClass5Source,
        terrain: &SessionTerrain,
    ) {
        let _ = model9_step(source, terrain);
    }
}

/// Dispatch the model-9 action cursor in the same order as the pinned source:
/// action 72 is only the countdown/transition callback, while action 73 owns
/// the movement and collision retry body.
fn model9_step<W: FlightWorld>(
    source: &mut Level2AuthoredClass5Source,
    world: &W,
) -> Option<Model9MoveResult> {
    match source.action {
        MODEL9_ACTION_COUNTDOWN => {
            model9_step_action72(source);
            None
        }
        MODEL9_ACTION_MOVEMENT => Some(model9_step_action73(source, world)),
        _ => None,
    }
}

/// Port `sub_20370`'s countdown. The zero test uses the value before the
/// decrement, so a source starting at 16 spends sixteen callbacks at action
/// 72 and transitions on the seventeenth callback.
fn model9_step_action72(source: &mut Level2AuthoredClass5Source) {
    let previous_countdown = source.dword_0x10;
    source.dword_0x10 = previous_countdown.wrapping_sub(1);
    if previous_countdown == 0 {
        // `sub_20F20` runs before the action/timer writes. These are the
        // source fields represented by the class-5 carrier; the remaining
        // presentation/target side effects are outside this model-local seam.
        source.actual_speed = source.minimum_speed;
        source.particle_index = MODEL9_MOVEMENT_PARTICLE;
        source.action = MODEL9_ACTION_MOVEMENT;
        source.dword_0x10 = MODEL9_MOVEMENT_COUNTDOWN;
        source.byte_0x46_70 = 0;
    }
}

fn model9_step_action73<W: FlightWorld>(
    source: &mut Level2AuthoredClass5Source,
    world: &W,
) -> Model9MoveResult {
    model9_step_movement(source, world)
}

/// Port the four candidate headings used by `sub_1B8C0`.
///
/// The third candidate deliberately preserves the source's split-byte write;
/// it is not equivalent to subtracting 341 from the original heading.
fn model9_retry_headings(yaw: i16) -> [i16; 4] {
    let yaw = i32::from(yaw);
    let first = yaw & MODEL9_YAW_MASK;
    let second = (yaw + 341) & MODEL9_YAW_MASK;
    let third = ((yaw.wrapping_sub(85) as u16 & 0x00ff)
        | ((((yaw.wrapping_sub(341) as u16) >> 8) & 7) << 8))
        & MODEL9_YAW_MASK as u16;
    let fourth = ((((yaw + 0x0400) as u16) & 0x0700) | (yaw as u8 as u16)) & MODEL9_YAW_MASK as u16;
    [
        i16::try_from(first).expect("model-9 retry heading fits i16"),
        i16::try_from(second).expect("model-9 retry heading fits i16"),
        i16::try_from(third).expect("model-9 retry heading fits i16"),
        i16::try_from(fourth).expect("model-9 retry heading fits i16"),
    ]
}

/// Exact shortest-angle turn used after a successful source candidate.
fn model9_turn_delta(from: i16, to: i16) -> i16 {
    let from = i32::from(from) & MODEL9_YAW_MASK;
    let to = i32::from(to) & MODEL9_YAW_MASK;
    let mut delta = to - from;
    if delta.abs() > 1024 {
        delta += if delta >= 0 { -2048 } else { 2048 };
    }
    if delta == 0 {
        return 0;
    }
    let magnitude = delta.abs().min(i32::from(MODEL9_TURN_LIMIT));
    i16::try_from(magnitude * delta.signum()).expect("model-9 turn delta fits i16")
}

fn model9_heading_after_turn(heading: i16, roll: i16) -> i16 {
    i16::try_from(
        (i32::from(heading) + i32::from(model9_turn_delta(heading, roll))) & MODEL9_YAW_MASK,
    )
    .expect("masked model-9 heading fits i16")
}

/// `sub_1B7A0`'s four-corner steep-terrain measure, expressed through the
/// typed terrain seam. Ground heights are stored in the source's 5-bit fixed
/// point scale, so the corner samples are shifted back before comparison.
fn model9_slope_measure<W: FlightWorld>(position: Position, world: &W) -> i32 {
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

/// The model-9 `sub_102D0(..., 1)` cave part. Particle 220 supplies a 125
/// unit fov and descriptor row 80 supplies the 512-unit ground clearance.
/// There is no material rejection for this descriptor.
fn model9_cave_collision_blocked<W: FlightWorld>(candidate: Position, world: &W) -> bool {
    let Some(ceiling) = world.ceiling_height(candidate) else {
        return false;
    };
    let tile = Tile {
        x: candidate.x.to_be_bytes()[0],
        y: candidate.y.to_be_bytes()[0],
    };
    world.angle_flags(tile) & 8 != 0
        || i32::from(world.ground_height(candidate)) + MODEL9_PARTICLE_FOV + MODEL9_CAVE_CLEARANCE
            > i32::from(ceiling)
}

fn model9_candidate_blocked<W: FlightWorld>(candidate: Position, world: &W) -> bool {
    // `sub_102D0` samples the candidate for model 9's zero-width/zero-depth
    // particle footprint. Keep the trait hook in the predicate so cave
    // worlds can provide their source-backed topology check as well as the
    // explicit descriptor clearance check below.
    world.cave_collision_blocked(candidate)
        || model9_cave_collision_blocked(candidate, world)
        || model9_slope_measure(candidate, world) >= MODEL9_SLOPE_LIMIT
}

fn model9_step_movement<W: FlightWorld>(
    source: &mut Level2AuthoredClass5Source,
    world: &W,
) -> Model9MoveResult {
    let before = source.position;
    let headings = model9_retry_headings(source.yaw);
    for heading in headings {
        source.yaw = heading;
        let mut candidate = before;
        crate::move_entity(&mut candidate, heading, 0, source.actual_speed);
        candidate.z = world.ground_height(candidate);
        let crossed_tile = before.x >> 8 != candidate.x >> 8 || before.y >> 8 != candidate.y >> 8;
        if !crossed_tile || !model9_candidate_blocked(candidate, world) {
            source.position = candidate;
            source.yaw = model9_heading_after_turn(heading, source.roll);
            return Model9MoveResult::Accepted {
                heading,
                position: candidate,
            };
        }
    }

    // Descriptor row 80 does not kill on a geometric all-blocked result, but
    // the source still applies its common water death rule at the origin.
    if world.material_mask(before) == WATER_AMBIENCE_MASK {
        source.life = -1;
    }
    Model9MoveResult::Blocked {
        heading: source.yaw,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alliance_promotes_legacy_skeleton_without_reallocating_its_source_address() {
        let mut session = crate::native_world::tests::level_two_campaign_session();
        let feature = session.session.state.live.mission.authored_features().iter()
            .find(|feature| feature.kind == 5 && feature.subtype == 9).copied().unwrap();
        let position = feature.world_position(session.canonical_terrain().ground_height(feature.world_position(0)));
        let particles = session.session.definition.presentation.particles.clone();
        let parameters = session.session.definition.presentation.entity_parameters.clone();
        let live = &mut session.session.state.live;
        let owner = live.core.state.entity_index;
        let source = live.economy.economy.allocate_authored_skeleton(feature.index, position, 1234, &live.terrain).unwrap();
        let slot = source.source_entity_index;
        let handle = live.economy.handle(slot).unwrap();
        let free = live.economy.economy.free_slots.clone();
        let phase_counter = live.economy.economy.class5_model9_constructor_phase;
        let links = (live.economy.map_chains.old_map_entity(slot), live.economy.map_chains.next_entity(slot));
        assert!(live.economy.promote_authored_skeleton(slot, &particles, &parameters[80]).unwrap());
        assert_eq!(live.economy.handle(slot), Some(handle));
        assert_eq!(live.economy.economy.free_slots, free);
        assert_eq!(live.economy.economy.class5_model9_constructor_phase, phase_counter);
        assert_eq!((live.economy.map_chains.old_map_entity(slot), live.economy.map_chains.next_entity(slot)), links);
        assert!(!live.economy.authored_class5().iter().any(|row| row.source_entity_index == slot));
        let row = &live.economy.entities[usize::from(slot)];
        assert_eq!((row.life, row.mana, row.entity_rng, row.position), (source.life, source.mana, source.entity_rng, source.position));
        let effect = crate::native_world::alliance_spell::construct_impact(&mut live.economy, &mut live.terrain).unwrap();
        let row = &mut live.economy.entities[usize::from(effect)];
        row.source_id = owner;
        row.record[74] = 32;
        row.subspell_index = 40;
        row.record[154..156].copy_from_slice(&slot.to_le_bytes());
        let impact = crate::native_world::alliance_spell::step_impact(&mut live.economy, effect, &[slot]).unwrap();
        assert_eq!(impact.affected_entities, [slot]);
        session.step_outer_turn(PlayerCommand::default(), 0);
        let row = &session.session.state.live.economy.entities[usize::from(slot)];
        assert_eq!(session.session.state.live.economy.handle(slot), Some(handle));
        assert_eq!(row.record[77], 14);
        assert!(session.cf10_production_schedule_receipts().iter().any(|receipt| matches!(
            receipt, crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot: visited, .. } if *visited == slot
        )));
        assert!(!session.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::Error { source_slot, .. } if *source_slot == slot)));
    }

    #[derive(Clone, Copy, Debug, Default)]
    struct Model9TestWorld {
        block_all: bool,
        material: u32,
        ceiling: Option<i16>,
        angles: u8,
        blocked_tile: Option<(u8, u8)>,
        steep_corner: Option<(u8, u8, i16)>,
    }

    impl FlightWorld for Model9TestWorld {
        fn ground_height(&self, position: Position) -> i16 {
            self.steep_corner
                .filter(|(x, y, _)| {
                    position.x.to_be_bytes()[0] == *x && position.y.to_be_bytes()[0] == *y
                })
                .map_or(0, |(_, _, height)| height)
        }

        fn ceiling_height(&self, _position: Position) -> Option<i16> {
            self.ceiling
        }

        fn material_mask(&self, _position: Position) -> u32 {
            self.material
        }

        fn angle_flags(&self, tile: Tile) -> u8 {
            self.angles
                | u8::from(self.block_all || self.blocked_tile == Some((tile.x, tile.y))) * 8
        }

        fn cave_collision_blocked(&self, _position: Position) -> bool {
            false
        }
    }

    fn model9_source(position: Position, yaw: i16, roll: i16) -> Level2AuthoredClass5Source {
        Level2AuthoredClass5Source {
            source_entity_index: 10,
            entity_rng: 0x1234,
            authored_index: 245,
            model: 9,
            action: MODEL9_ACTION_COUNTDOWN,
            minimum_speed: 20,
            maximum_speed: 0,
            actual_speed: 20,
            max_life: 1_000,
            life: 1_000,
            mana: 500,
            max_mana: 0,
            subspell_index: 500,
            dword_0x10: 16,
            pending_word_0x62: 0,
            word_0x26: 0,
            word_0x24: 0,
            byte_0x3b: 0,
            word_0x96_150: 0,
            byte_0x46_70: 0,
            particle_index: 220,
            source_byte_3e: 0,
            source_byte_39: 0,
            yaw,
            pitch: 0,
            roll,
            fov: 0,
            shift_rot_x: 128,
            shift_rot_y: 128,
            position,
            phase: 0,
            drawing_enabled: true,
            active: true,
        }
    }

    #[test]
    fn level_two_model9_action72_matches_collision_and_retry_order() {
        assert_eq!(model9_retry_headings(0), [0, 341, 1707, 1024]);

        let mut world = Model9TestWorld::default();
        let mut source = model9_source(
            Position {
                x: 0x00f0,
                y: 0x01f0,
                z: 0,
            },
            0,
            1024,
        );
        source.actual_speed = 512;
        world.ceiling = Some(2_000);
        let initial_position = source.position;
        let initial_yaw = source.yaw;

        // `sub_20370` decrements action 72 without moving. Its zero test is
        // against the pre-decrement value, so the 16th tick reaches zero but
        // remains action 72; only the next tick enters action 73.
        for expected_countdown in (0..16).rev() {
            assert_eq!(model9_step(&mut source, &world), None);
            assert_eq!(source.action, MODEL9_ACTION_COUNTDOWN);
            assert_eq!(source.dword_0x10, expected_countdown);
            assert_eq!(source.position, initial_position);
            assert_eq!(source.yaw, initial_yaw);
        }
        assert_eq!(model9_step(&mut source, &world), None);
        assert_eq!(source.action, MODEL9_ACTION_MOVEMENT);
        assert_eq!(source.dword_0x10, MODEL9_MOVEMENT_COUNTDOWN);
        assert_eq!(source.actual_speed, source.minimum_speed);
        assert_eq!(source.particle_index, MODEL9_MOVEMENT_PARTICLE);
        assert_eq!(source.byte_0x46_70, 0);
        assert_eq!(source.position, initial_position);
        assert_eq!(source.yaw, initial_yaw);

        // Action 73 owns the movement body. Block the first candidate tile so
        // the source's second candidate is the committed retry.
        let mut movement_source = model9_source(initial_position, 0, 1024);
        movement_source.action = MODEL9_ACTION_MOVEMENT;
        movement_source.dword_0x10 = MODEL9_MOVEMENT_COUNTDOWN;
        movement_source.actual_speed = 512;
        let mut first_position = movement_source.position;
        crate::move_entity(&mut first_position, 0, 0, 512);
        world.blocked_tile = Some((
            first_position.x.to_be_bytes()[0],
            first_position.y.to_be_bytes()[0],
        ));
        let expected_heading = model9_retry_headings(0)[1];
        let mut expected_position = movement_source.position;
        crate::move_entity(&mut expected_position, expected_heading, 0, 512);
        expected_position.z = 0;
        let result = model9_step(&mut movement_source, &world);
        assert_eq!(
            result,
            Some(Model9MoveResult::Accepted {
                heading: expected_heading,
                position: expected_position,
            })
        );
        assert_eq!(movement_source.position, expected_position);
        assert_eq!(movement_source.yaw, expected_heading + MODEL9_TURN_LIMIT);

        // A 70-point corner delta is the descriptor's steep-terrain boundary.
        world.blocked_tile = None;
        world.steep_corner = Some((2, 1, 2_240));
        assert!(model9_candidate_blocked(
            Position {
                x: 0x0200,
                y: 0x0100,
                z: 0,
            },
            &world,
        ));

        // The cave angle bit and the source's fov+clearance envelope are
        // independent rejection paths.
        world.steep_corner = None;
        world.ceiling = Some(600);
        assert!(model9_candidate_blocked(
            Position {
                x: 0x0200,
                y: 0x0100,
                z: 0,
            },
            &world,
        ));
        world.ceiling = Some(2_000);
        world.angles = 8;
        assert!(model9_candidate_blocked(
            Position {
                x: 0x0200,
                y: 0x0100,
                z: 0,
            },
            &world,
        ));

        // All candidates blocked leaves the source in place, retains the
        // fourth attempted yaw, and applies the source water death rule.
        let mut blocked_source = model9_source(
            Position {
                x: 0x00f0,
                y: 0x01f0,
                z: 0,
            },
            0,
            0,
        );
        blocked_source.actual_speed = 512;
        blocked_source.life = 7;
        world.block_all = true;
        world.material = WATER_AMBIENCE_MASK;
        let blocked_position = blocked_source.position;
        let result = model9_step_movement(&mut blocked_source, &world);
        assert_eq!(
            result,
            Model9MoveResult::Blocked {
                heading: model9_retry_headings(0)[3],
            }
        );
        assert_eq!(blocked_source.position, blocked_position);
        assert_eq!(blocked_source.yaw, model9_retry_headings(0)[3]);
        assert_eq!(blocked_source.life, -1);
    }
}
