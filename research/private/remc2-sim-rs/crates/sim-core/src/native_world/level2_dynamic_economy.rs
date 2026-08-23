use super::*;

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
const POSSESSION_TARGET_Z_SHIFT: i16 = 28;
/// Source `sub_69640`'s tiered Possession constructor values. The direct
/// source profile supplies the physical tier; this arena helper only carries
/// the constructor identity and its `byte_0x44` selector into the live row.
const POSSESSION_TIERED_PROJECTILE_MODEL: u8 = 17;
const POSSESSION_TIERED_PROJECTILE_ACTION: u8 = 18;

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

fn level2_castle_auxiliary_settle(position: &mut Position, terrain: &SessionTerrain) {
    // Descriptor 83 supplies word 0x0c = 0 and word 0x0e = -256 to
    // `sub_1EEE0 -> sub_580E0`.
    let ground = terrain.ground_height(*position);
    if position.z > ground {
        position.z = position.z.wrapping_sub(256);
    }
    if position.z <= ground {
        position.z = ground;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Level2CastleAuxiliaryTarget {
    source_entity_index: u16,
    source_id: u16,
    class: u8,
    model: u8,
    life: i32,
    control_flags: u32,
    position: Position,
    imported: bool,
}

/// Fixed linked-list memberships rebuilt by `UpdateEntities_57730` before
/// its ascending callback walk. Source callbacks read the pointed-to rows
/// live, but rows born later in the same walk cannot join either list until
/// the following rebuild.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Level2CastleAuxiliaryMembership {
    replacement_list_epoch: u32,
    class3_source_slots: Vec<crate::campaign::source_arena::SourceEntityHandle>,
    model15_source_slots: Vec<crate::campaign::source_arena::SourceEntityHandle>,
}

fn level2_castle_auxiliary_probe_blocked(terrain: &SessionTerrain, candidate: Position) -> bool {
    // `sub_102D0(..., 1)` samples this candidate once for descriptor 83.
    // Particle 0 contributes a 125-unit FOV to the Cave ceiling test, while
    // descriptor 83's dword 0x14 admits only the 0x0002_0000 material bit.
    if terrain.material_mask(candidate) & !LEVEL2_CASTLE_AUXILIARY_ALLOWED_MATERIALS != 0 {
        return true;
    }
    if terrain.map_type == MapType::Cave {
        let tile = crate::Tile {
            x: candidate.x.to_be_bytes()[0],
            y: candidate.y.to_be_bytes()[0],
        };
        if terrain.angle_flags(tile) & 8 != 0 {
            return true;
        }
        if let Some(ceiling) = terrain.ceiling_height(candidate) {
            let ground = terrain.ground_height(candidate);
            if i32::from(ground) + 125 > i32::from(ceiling) {
                return true;
            }
        }
    }
    false
}

fn level2_castle_auxiliary_roam(
    child: &mut Level2CastleChild,
    peers: &[Level2CastleAuxiliaryTarget],
    terrain: &SessionTerrain,
) {
    if child.source_byte_3e % 8 == 0 {
        if terrain.material_mask(child.position) & !LEVEL2_CASTLE_AUXILIARY_ALLOWED_MATERIALS != 0 {
            child.action = 124;
            return;
        }
        let score_moduli = [0x1b58_u16, 0x1b58, 10, 0x1b58];
        let mut best_score = 1_u16;
        let mut heading = child.yaw;
        for modulus in score_moduli {
            let mut candidate = child.position;
            crate::move_entity(
                &mut candidate,
                i16::try_from(heading).expect("auxiliary heading fits"),
                0,
                256,
            );
            child.entity_rng = level2_next_rng16(child.entity_rng);
            let score = if level2_castle_auxiliary_probe_blocked(terrain, candidate) {
                0
            } else {
                child.entity_rng % modulus + 2
            };
            if score > best_score {
                best_score = score;
                child.yaw = heading;
            }
            heading = heading.wrapping_add(0x0200) & 0x07ff;
        }
    }

    let mut predicted = child.position;
    if child.source_byte_3e % 16 == 0 {
        match (child.yaw.wrapping_sub(256) >> 9) & 3 {
            0 | 2 => predicted.y = (predicted.y >> 8 << 8).wrapping_add(128),
            1 | 3 => predicted.x = (predicted.x >> 8 << 8).wrapping_add(128),
            _ => unreachable!("two-bit auxiliary direction"),
        }
    }
    if let Some(peer) = peers
        .iter()
        .filter(|peer| {
            peer.source_id != child.id_entity_index
                && child.position.x.abs_diff(peer.position.x) < 256
                && child.position.y.abs_diff(peer.position.y) < 256
        })
        .min_by_key(|peer| peer.source_entity_index)
    {
        child.roll = u16::try_from(heading_for_positions(peer.position, child.position))
            .expect("auxiliary peer heading fits");
    }
    let move_now = if child.roll == child.yaw {
        true
    } else {
        child.entity_rng = level2_next_rng16(child.entity_rng);
        child.entity_rng % 20 <= 10
    };
    if move_now {
        crate::move_entity(
            &mut predicted,
            i16::try_from(child.yaw).expect("auxiliary yaw fits"),
            0,
            child.speed,
        );
        child.position = predicted;
    }
    level2_castle_auxiliary_settle(&mut child.position, terrain);
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
    pub position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub axis_position: Position,
    pub source_id_entity_index: u16,
    pub holder_entity_index: u16,
    pub source_subspell_index: u16,
    pub source_mana: i32,
    pub source_dword_0x10: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Level2FireballStep {
    pub live: bool,
    pub impact_sound: bool,
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
        let entity_rng = self.new_event_rng(source_entity_index);
        let projectile = Level2FireballProjectile {
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
            actual_speed: FIREBALL_SPEED
                .wrapping_add(player_speed)
                .clamp(FIREBALL_SPEED, 0x2000),
            minimum_speed: FIREBALL_SPEED,
            life: FIREBALL_LIFETIME_TICKS,
            max_life: FIREBALL_LIFETIME_TICKS,
            dword_0x10: payload.source_dword_0x10,
            control_flags: 0,
            source_word_2c: 0,
            phase: 0,
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

    /// Submit one source-shaped Fireball damage mailbox write. The target is
    /// resolved by class/model/action and live map relation at write time, and
    /// an already occupied attacker word is rejected so a one-shot projectile
    /// cannot duplicate its hit before the Bee callback consumes it. The
    /// source-shaped damage dword is intentionally retained after consumption.
    pub(super) fn submit_authored_bee_damage(
        &mut self,
        target_slot: u16,
        damage: i32,
        attacker: u16,
    ) -> bool {
        if damage <= 0 || attacker == 0 || !self.map_chains.is_linked(target_slot) {
            return false;
        }
        let Some(bee) = self.authored_bees.iter_mut().find(|bee| {
            bee.active
                && bee.drawing_enabled
                && bee.life >= 0
                && bee.source_entity_index == target_slot
                && bee.entity_id != attacker
                && bee.source_class == 5
                && bee.model == 2
                && bee.action == 17
                && bee.pending_attacker_entity_index == 0
        }) else {
            return false;
        };
        bee.pending_damage = damage;
        bee.pending_attacker_entity_index = attacker;
        true
    }

    /// Source `sub_30D50` callback. Collision is attempted once on the first
    /// non-delayed callback, before the callback RNG/audio tail; the hidden row
    /// remains map-linked until the following source boundary reclaims it.
    pub(super) fn step_fireball_projectile_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
    ) -> Level2FireballStep {
        let Some(projectile_index) = self.fireball_projectiles.iter().position(|projectile| {
            projectile.active && projectile.source_entity_index == source_slot
        }) else {
            return Level2FireballStep {
                live: false,
                impact_sound: false,
            };
        };
        let mut projectile = self.fireball_projectiles[projectile_index];
        projectile.phase = projectile.phase.wrapping_add(1);
        let first_callback = projectile.control_flags & 0x2 == 0;
        if projectile.dword_0x10 & 3 != 0 {
            projectile.dword_0x10 = projectile.dword_0x10.wrapping_sub(1);
        } else {
            projectile.life = projectile.life.wrapping_sub(1);
            if projectile.life < -1 {
                projectile.active = false;
                projectile.drawing_disabled = true;
            } else {
                crate::move_entity(
                    &mut projectile.position,
                    projectile.yaw,
                    projectile.pitch,
                    projectile.actual_speed,
                );
                if projectile.control_flags & 0x2 == 0 {
                    // The source's byte-2 bit 0 suppresses the damage scan
                    // while still allowing the one-shot callback tail below.
                    // The typed row keeps that source byte in the high byte
                    // of the existing control word; ordinary Fireball rows
                    // are constructed with it clear.
                    if projectile.control_flags & 0x0001_0000 == 0 {
                        let target = self
                            .authored_bees
                            .iter()
                            .filter(|bee| {
                                bee.active
                                    && bee.drawing_enabled
                                    && bee.life >= 0
                                    && bee.source_class == 5
                                    && bee.model == 2
                                    && bee.action == 17
                                    && bee.source_entity_index != projectile.owner_entity_index
                                    && bee.entity_id != projectile.owner_entity_index
                                    && bee.pending_attacker_entity_index == 0
                                    && self.map_chains.is_linked(bee.source_entity_index)
                                    && crate::level1_mission::source_entities_overlap(
                                        projectile.position,
                                        LEVEL2_FIREBALL_COLLISION_HALF_WIDTH,
                                        LEVEL2_FIREBALL_COLLISION_VERTICAL_OFFSET,
                                        LEVEL2_FIREBALL_COLLISION_HALF_HEIGHT,
                                        bee.position,
                                        LEVEL2_BEE_COLLISION_HALF_EXTENT,
                                        LEVEL2_BEE_GROUND_CLEARANCE,
                                        LEVEL2_BEE_COLLISION_HALF_EXTENT,
                                    )
                            })
                            .min_by_key(|bee| bee.source_entity_index)
                            .map(|bee| bee.source_entity_index);
                        if let Some(target_slot) = target {
                            let _ = self.submit_authored_bee_damage(
                                target_slot,
                                projectile.payload,
                                projectile.owner_entity_index,
                            );
                        }
                    }
                    projectile.control_flags |= 0x2;
                    projectile.entity_rng = level2_next_rng16(projectile.entity_rng);
                    projectile.source_word_2c = i16::try_from(projectile.entity_rng % 0x41)
                        .expect("Fireball RNG fits")
                        - 32;
                }
                // `sub_580E0(position, ground, 0, 0, word_0x2c)` runs on
                // every non-delayed live callback, including callbacks after
                // the one-shot collision/audio gate has closed.
                let ground = terrain.ground_height(projectile.position);
                if projectile.position.z > ground {
                    projectile.position.z = projectile
                        .position
                        .z
                        .wrapping_add(projectile.source_word_2c);
                }
                if projectile.position.z <= ground {
                    projectile.position.z = ground;
                }
                let _ = self.map_chains.sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: true,
                    x: projectile.position.x,
                    y: projectile.position.y,
                });
            }
        }
        self.source_rng.insert(source_slot, projectile.entity_rng);
        self.fireball_projectiles[projectile_index] = projectile;
        Level2FireballStep {
            live: projectile.active,
            impact_sound: projectile.active
                && first_callback
                && projectile.control_flags & 0x2 != 0,
        }
    }

    /// Source `NewAdd0A0C_4E8C0` product for a successful possession flyer
    /// collision. This is intentionally private to the projectile callback:
    /// public production routes cannot create an ownership pulse directly.
    fn allocate_possession_effect(
        &mut self,
        projectile: Level2PossessionProjectile,
    ) -> Option<u16> {
        let source_entity_index = self.allocate_slot()?;
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
    pub fn step_possession_projectile_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(projectile_index) = self.possession_projectiles.iter().position(|projectile| {
            projectile.active && projectile.source_entity_index == source_slot
        }) else {
            return false;
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

        let target =
            level2_first_possession_target_sphere_slot(&self.map_chains, &self.spheres, moved);
        if let Some(target_sphere_slot) = target {
            if let Some(target) = self
                .spheres
                .iter()
                .find(|sphere| sphere.active && sphere.source_entity_index == target_sphere_slot)
            {
                moved.position = target.position;
                moved.position.z = moved.position.z.wrapping_add(POSSESSION_TARGET_Z_SHIFT);
            }
            self.possession_projectiles[projectile_index] = moved;
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: moved.position.x,
                y: moved.position.y,
            });
            if self.allocate_possession_effect(moved).is_some() {
                let projectile = &mut self.possession_projectiles[projectile_index];
                projectile.active = false;
                return false;
            }
            return true;
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
        if terminal && self.allocate_possession_effect(moved).is_some() {
            self.possession_projectiles[projectile_index].active = false;
            return false;
        }
        true
    }

    /// Source `PossesHitMana_320E0`: every live model-12 callback walks its
    /// complete map-search square and writes every qualifying model-39
    /// mailbox through strict overlap, then advances phase and applies the
    /// signed `life-- < 0` terminal test.
    pub fn step_possession_effect_slot(&mut self, source_slot: u16) -> bool {
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
                let Some(sphere_index) = self.spheres.iter().position(|sphere| {
                    sphere.active
                        && !sphere.pending_delete
                        && sphere.source_entity_index == candidate_slot
                        && u16::try_from(sphere.id).ok() != Some(effect.source_id_entity_index)
                }) else {
                    continue;
                };
                let sphere = self.spheres[sphere_index];
                let dimension = level2_mana_sphere_dimension(sphere.mana);
                if crate::level1_mission::source_entities_overlap(
                    effect.position,
                    POSSESSION_IMPACT_COLLISION_HALF_WIDTH,
                    POSSESSION_IMPACT_COLLISION_VERTICAL_OFFSET,
                    POSSESSION_IMPACT_COLLISION_HALF_HEIGHT,
                    sphere.position,
                    dimension,
                    dimension,
                    dimension,
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
    pub(super) fn reclaim_fireball_rows_at_source_boundary(&mut self) {
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
    /// enabled, mana is split into `clamp(mana / 1000, 1, 16)` spheres and the
    /// final sphere receives the exact remainder. The source writes the dying
    /// entity RNG before attempting allocations, so an exhausted arena may
    /// return a partial vector while preserving the already-created spheres.
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
        let fraction = if use_mana_fraction {
            (mana / 1_000).clamp(1, 16)
        } else {
            1
        };
        let each_mana = mana / fraction;
        let mut sphere_ids = Vec::with_capacity(usize::try_from(fraction).unwrap_or(1));
        for sphere_index in 0..fraction {
            let Some(sphere_slot) = self.allocate_slot() else {
                break;
            };
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
            let sphere_mana = if sphere_index + 1 == fraction {
                mana - each_mana * (fraction - 1)
            } else {
                each_mana
            };
            let sphere = ManaSphere {
                id: u32::from(sphere_slot),
                source_entity_index: sphere_slot,
                entity_rng: speed_rng,
                position,
                velocity_x: velocity.x.cast_signed(),
                velocity_y: velocity.y.cast_signed(),
                vertical_velocity,
                yaw,
                mana: sphere_mana,
                owner_entity_index,
                player_entity_index: owner_entity_index,
                incoming_owner_entity_index: 0,
                incoming_possession_payload: 0,
                attraction_source_entity_index: 0,
                attraction_step: 0,
                attraction_distance_high: 0,
                assigned_carrier_id: 0,
                assigned_defender_slot: 0,
                control_flags: 0,
                phase: sphere_slot as u8,
                active: true,
                pending_delete: false,
            };
            sphere_ids.push(sphere.id);
            self.spheres.push(sphere);
            self.refresh_owner_mana_rows();
            self.events.push(Level2EconomyEvent::ManaSphereSpawned {
                sphere_id: sphere.id,
                source_entity_index,
                mana: sphere_mana,
            });
        }
        sphere_ids
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
            control_flags: SOURCE_REPLACEABLE_FLAG,
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
    pub(super) fn reclaim_death_burst_rows_at_source_boundary(&mut self) {
        let reclaimed = self
            .death_bursts
            .iter()
            .filter(|burst| !burst.active)
            .map(|burst| (burst.source_entity_index, burst.entity_rng))
            .collect::<Vec<_>>();
        self.death_bursts.retain(|burst| burst.active);
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
                && ((child.source_class == 5 && child.model == 15)
                    || (child.source_class == 10 && matches!(child.model, 41 | 42 | 43 | 79)))
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

    fn consume_castle_auxiliary_damage(&mut self, child_index: usize) -> u8 {
        let mut state = 0_u8;
        let pending_attacker = self.castle_children[child_index].pending_attacker_entity_index;
        if pending_attacker != 0 {
            let damage = self.castle_children[child_index].pending_damage;
            let child = &mut self.castle_children[child_index];
            child.life = child.life.wrapping_sub(damage);
            child.pending_attacker_entity_index = 0;
            child.damage_attacker_entity_index = pending_attacker;
            state = 1;
        } else {
            self.castle_children[child_index].damage_attacker_entity_index = 0;
        }

        let mut linked_slot = self.castle_children[child_index].chain_next_slot;
        let mut visited = BTreeSet::new();
        while linked_slot != 0 && visited.insert(linked_slot) {
            let Some(linked) = self
                .castle_children
                .iter()
                .find(|candidate| candidate.active && candidate.source_entity_index == linked_slot)
                .copied()
            else {
                break;
            };
            if linked.life < self.castle_children[child_index].life {
                let child = &mut self.castle_children[child_index];
                child.life = linked.life;
                child.damage_attacker_entity_index = linked.damage_attacker_entity_index;
                state = 1;
                break;
            }
            linked_slot = linked.chain_next_slot;
        }
        if self.castle_children[child_index].life < 0 {
            state = 2;
            self.castle_children[child_index].killer_entity_index =
                self.castle_children[child_index].damage_attacker_entity_index;
        }
        state
    }

    fn castle_auxiliary_target_at(
        &self,
        source_slot: u16,
        entities: &[EntitySnapshot],
    ) -> Option<Level2CastleAuxiliaryTarget> {
        if let Some(castle) = self
            .castle
            .filter(|castle| castle.source_entity_index == source_slot)
        {
            return Some(Level2CastleAuxiliaryTarget {
                source_entity_index: source_slot,
                source_id: castle.source_id_entity_index,
                class: castle.source_class,
                model: castle.source_model,
                life: castle.life,
                control_flags: castle.control_flags,
                position: castle.position,
                imported: false,
            });
        }
        if let Some(child) = self
            .castle_children
            .iter()
            .find(|child| child.source_entity_index == source_slot)
        {
            return Some(Level2CastleAuxiliaryTarget {
                source_entity_index: source_slot,
                source_id: child.id_entity_index,
                class: child.source_class,
                model: child.model,
                life: child.life,
                control_flags: child.control_flags,
                position: child.position,
                imported: false,
            });
        }
        if let Some(carrier) = self
            .carriers
            .iter()
            .find(|carrier| carrier.source_entity_index == source_slot)
        {
            return Some(Level2CastleAuxiliaryTarget {
                source_entity_index: source_slot,
                source_id: carrier.owner_entity_index,
                class: 3,
                model: 3,
                life: carrier.life,
                control_flags: 0,
                position: carrier.position,
                imported: false,
            });
        }
        // A generation entry means NewEvent has owned this fixed source slot.
        // If no typed row remains, do not fall through to the stale imported
        // reset projection that the allocation replaced.
        if self.slot_generations.contains_key(&source_slot) {
            return None;
        }
        entities
            .get(usize::from(source_slot))
            .map(|entity| Level2CastleAuxiliaryTarget {
                source_entity_index: source_slot,
                source_id: entity.source_id,
                class: entity.class,
                model: entity.model,
                life: entity.life,
                control_flags: entity.control_flags,
                position: entity.position,
                imported: true,
            })
    }

    pub(crate) fn castle_auxiliary_membership(
        &self,
        entities: &[EntitySnapshot],
    ) -> Level2CastleAuxiliaryMembership {
        let mut class3_source_slots = entities
            .iter()
            .filter(|entity| {
                entity.active
                    && entity.class == 3
                    && entity.life >= 0
                    && !self.slot_generations.contains_key(&entity.index)
            })
            .map(|entity| entity.index)
            .collect::<BTreeSet<_>>();
        if let Some(castle) = self
            .castle
            .filter(|castle| castle.active && castle.source_class == 3 && castle.life >= 0)
        {
            class3_source_slots.insert(castle.source_entity_index);
        }
        class3_source_slots.extend(
            self.castle_children
                .iter()
                .filter(|child| child.active && child.source_class == 3 && child.life >= 0)
                .map(|child| child.source_entity_index),
        );
        class3_source_slots.extend(
            self.carriers
                .iter()
                .filter(|carrier| carrier.active && carrier.life >= 0)
                .map(|carrier| carrier.source_entity_index),
        );
        let model15_source_slots = self
            .castle_children
            .iter()
            .filter(|child| {
                child.active
                    && child.life >= 0
                    && child.source_class == 5
                    && child.model == 15
                    && !matches!(child.action, 180 | 232 | 234)
            })
            .map(|child| child.source_entity_index)
            .collect::<BTreeSet<_>>();
        Level2CastleAuxiliaryMembership {
            replacement_list_epoch: self.replacement_list_epoch,
            class3_source_slots: class3_source_slots
                .into_iter()
                .map(
                    |source_slot| crate::campaign::source_arena::SourceEntityHandle {
                        source_slot,
                        generation: self.slot_generation(source_slot),
                    },
                )
                .collect(),
            model15_source_slots: model15_source_slots
                .into_iter()
                .map(
                    |source_slot| crate::campaign::source_arena::SourceEntityHandle {
                        source_slot,
                        generation: self.slot_generation(source_slot),
                    },
                )
                .collect(),
        }
    }

    fn castle_auxiliary_target_scan(
        &self,
        child: Level2CastleChild,
        entities: &[EntitySnapshot],
        membership: &Level2CastleAuxiliaryMembership,
    ) -> Option<u16> {
        if membership.replacement_list_epoch != self.replacement_list_epoch {
            return None;
        }
        let range_squared =
            LEVEL2_CASTLE_AUXILIARY_TARGET_RANGE.wrapping_mul(LEVEL2_CASTLE_AUXILIARY_TARGET_RANGE);
        let mut best_slot = None;
        let mut best_distance = u32::MAX;
        for handle in &membership.class3_source_slots {
            if self.slot_generation(handle.source_slot) != handle.generation {
                continue;
            }
            let Some(target) = self.castle_auxiliary_target_at(handle.source_slot, entities) else {
                continue;
            };
            if target.source_id == child.id_entity_index || target.control_flags & 0x20 != 0 {
                continue;
            }
            let distance = level2_wrapping_xy_distance(child.position, target.position);
            let heading = heading_for_positions(child.position, target.position);
            if distance <= range_squared
                && crate::angle_distance(i32::from(child.yaw), heading)
                    < LEVEL2_CASTLE_AUXILIARY_TARGET_HALF_ANGLE
                && distance < best_distance
            {
                best_slot = Some(target.source_entity_index);
                best_distance = distance;
            }
        }
        best_slot
    }

    fn castle_auxiliary_live_peers(
        &self,
        entities: &[EntitySnapshot],
        membership: &Level2CastleAuxiliaryMembership,
    ) -> Vec<Level2CastleAuxiliaryTarget> {
        if membership.replacement_list_epoch != self.replacement_list_epoch {
            return Vec::new();
        }
        membership
            .model15_source_slots
            .iter()
            .filter(|handle| self.slot_generation(handle.source_slot) == handle.generation)
            .filter_map(|handle| self.castle_auxiliary_target_at(handle.source_slot, entities))
            .collect()
    }

    fn aim_castle_auxiliary(&mut self, child_index: usize) {
        let child = &mut self.castle_children[child_index];
        child.entity_rng = level2_next_rng16(child.entity_rng);
        child.speed = 0;
        child.visual_index = if child.entity_rng % 20 <= 10 { 206 } else { 1 };
    }

    fn reset_castle_auxiliary_to_roam(&mut self, child_index: usize) {
        let child = &mut self.castle_children[child_index];
        child.speed = LEVEL2_CASTLE_AUXILIARY_MINIMUM_SPEED;
        child.visual_index = 0;
    }

    fn finish_self_replaced_castle_auxiliary_projectile(
        &mut self,
        source_slot: u16,
        target: Level2CastleAuxiliaryTarget,
        terrain: &SessionTerrain,
    ) {
        let Some(child) = self
            .authored_class5_archer_children
            .iter_mut()
            .find(|child| child.active && child.source_entity_index == source_slot)
        else {
            return;
        };
        // NewEvent overwrote the aliased `a1x` record before sub_23E60's
        // post-constructor copies. Those reads therefore observe the new
        // row's defaults, not the former model-15 fields.
        child.entity_id = source_slot;
        child.parent_entity_id = source_slot;
        child.word_0x96 = 0;
        child.xtype = u8::MAX;
        child.xsubtype = u8::MAX;
        // sub_23E60 retained only a pointer to the old a1x position. NewEvent
        // memsets the aliased fixed row before AddEventToMap reads it, so the
        // replacement is born at zero and is aimed from that zero position.
        child.position = Position::default();
        child.position.z = child
            .position
            .z
            .wrapping_add(level2_class5::archer::ARCHER_CHILD_PARTICLE_FOV / 2);
        child.yaw = i16::try_from(heading_for_positions(Position::default(), target.position))
            .expect("self-replaced auxiliary projectile heading fits");
        let horizontal = crate::legacy_sqrt(level2_wrapping_xy_distance(
            Position::default(),
            target.position,
        ));
        child.pitch = crate::pitch_for_delta(
            i32::try_from(horizontal).expect("self-replaced horizontal distance fits"),
            i32::from(target.position.z),
        );

        // `sub_1EEE0` now sees NewEvent descriptor 59 (wordC=0,
        // wordE=-4), then LABEL_26 sees action 13 and runs sub_24150 on the
        // replacement: minimum speed plus particle-0 presentation.
        let ground = terrain.ground_height(child.position);
        if child.position.z > ground {
            child.position.z = child.position.z.wrapping_sub(4);
        }
        if child.position.z <= ground {
            child.position.z = ground;
        }
        child.actual_speed = child.minimum_speed;
        child.particle_index = 0;
        child.shift_rot_x = 0;
        child.shift_rot_y = 0;
        child.fov = level2_class5::archer::ARCHER_POSE_FOV;
    }

    fn finish_castle_auxiliary_source_visit(&mut self, source_slot: u16) -> bool {
        if let Some(child) = self
            .castle_children
            .iter_mut()
            .find(|child| child.active && child.source_entity_index == source_slot)
        {
            child.source_byte_3e = child.source_byte_3e.wrapping_add(1);
            let child = *child;
            self.source_rng.insert(source_slot, child.entity_rng);
            self.sync_castle_child_map(child);
            return true;
        }
        if let Some(child) = self
            .authored_class5_archer_children
            .iter_mut()
            .find(|child| child.active && child.source_entity_index == source_slot)
        {
            child.source_byte_3e = child.source_byte_3e.wrapping_add(1);
            let child = *child;
            self.source_rng.insert(source_slot, child.entity_rng);
            return self
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: true,
                    x: child.position.x,
                    y: child.position.y,
                })
                .is_ok();
        }
        if let Some(burst) = self
            .death_bursts
            .iter_mut()
            .find(|burst| burst.source_entity_index == source_slot)
        {
            burst.source_byte_3e = burst.source_byte_3e.wrapping_add(1);
            self.source_rng.insert(source_slot, burst.entity_rng);
            return true;
        }
        false
    }

    fn spawn_castle_auxiliary_projectile(
        &mut self,
        child: Level2CastleChild,
        target: Level2CastleAuxiliaryTarget,
    ) -> Option<u16> {
        let fov = if child.visual_index == 206 { 120 } else { 125 };
        self.allocate_castle_auxiliary_projectile_packet(
            level2_class5::archer::ArcherChildRequest {
                parent_entity_index: 0,
                parent_entity_id: child.id_entity_index,
                parent_word_0x96: child.target_sphere_slot,
                parent_roll: 0,
                parent_position: child.position,
                parent_fov: fov,
                target_entity_index: target.source_entity_index,
                target_class: target.class,
                target_model: target.model,
                target_position: target.position,
            },
        )
        .map(|packet| packet.source_entity_index)
    }

    fn step_castle_auxiliary_action121(
        &mut self,
        child_index: usize,
        entities: &[EntitySnapshot],
        membership: &Level2CastleAuxiliaryMembership,
        terrain: &SessionTerrain,
    ) {
        let damage_state = self.consume_castle_auxiliary_damage(child_index);
        match damage_state {
            0 => {
                let peers = self.castle_auxiliary_live_peers(entities, membership);
                level2_castle_auxiliary_roam(
                    &mut self.castle_children[child_index],
                    &peers,
                    terrain,
                );
                let child = self.castle_children[child_index];
                if child.source_byte_3e % LEVEL2_CASTLE_AUXILIARY_CADENCE == 0
                    && child.source_byte_39 != 0
                    && let Some(target_slot) =
                        self.castle_auxiliary_target_scan(child, entities, membership)
                {
                    self.castle_children[child_index].action = 122;
                    self.castle_children[child_index].target_sphere_slot = target_slot;
                }
            }
            1 => {
                let attacker = self.castle_children[child_index].damage_attacker_entity_index;
                if self
                    .castle_auxiliary_target_at(attacker, entities)
                    .is_some_and(|entity| {
                        entity.class == 3
                            && entity.source_id != self.castle_children[child_index].id_entity_index
                    })
                {
                    self.castle_children[child_index].action = 122;
                    self.castle_children[child_index].target_sphere_slot = attacker;
                }
                level2_castle_auxiliary_settle(
                    &mut self.castle_children[child_index].position,
                    terrain,
                );
            }
            2 => self.castle_children[child_index].action = 124,
            _ => unreachable!("three-state auxiliary damage result"),
        }
        if self.castle_children[child_index].action == 122 {
            self.aim_castle_auxiliary(child_index);
        }
    }

    fn step_castle_auxiliary_action122(
        &mut self,
        child_index: usize,
        entities: &mut [EntitySnapshot],
        terrain: &SessionTerrain,
    ) {
        let damage_state = self.consume_castle_auxiliary_damage(child_index);
        if damage_state == 2 {
            self.castle_children[child_index].action = 124;
            self.reset_castle_auxiliary_to_roam(child_index);
            return;
        }

        let target_slot = self.castle_children[child_index].target_sphere_slot;
        let Some(target) = self.castle_auxiliary_target_at(target_slot, entities) else {
            self.castle_children[child_index].action = 121;
            self.reset_castle_auxiliary_to_roam(child_index);
            return;
        };
        // `sub_23E60` resolves word96 and updates roll before it checks the
        // target's life/draw-disable state. A retained hidden row therefore
        // still affects this write during the pass in which it was hidden.
        if self.castle_children[child_index].source_byte_3e & 3 == 0 {
            self.castle_children[child_index].roll = u16::try_from(heading_for_positions(
                self.castle_children[child_index].position,
                target.position,
            ))
            .expect("auxiliary target heading fits");
        }
        if target.life < 0 || target.control_flags & ENTITY_DRAW_DISABLED_BIT != 0 {
            self.castle_children[child_index].action = 121;
            self.reset_castle_auxiliary_to_roam(child_index);
            return;
        }
        if self.castle_children[child_index].source_byte_3e % LEVEL2_CASTLE_AUXILIARY_CADENCE == 0 {
            if crate::legacy_sqrt(level2_wrapping_source_distance_3d(
                self.castle_children[child_index].position,
                target.position,
            )) >= LEVEL2_CASTLE_AUXILIARY_TARGET_RANGE
            {
                self.castle_children[child_index].action = 121;
                self.reset_castle_auxiliary_to_roam(child_index);
                return;
            }
            let child = self.castle_children[child_index];
            if let Some(projectile_slot) = self.spawn_castle_auxiliary_projectile(child, target) {
                if target.imported
                    && let Some(target) = entities.get_mut(usize::from(target.source_entity_index))
                {
                    target.apply_source_sub_5ef70();
                }
                if projectile_slot == child.source_entity_index {
                    self.finish_self_replaced_castle_auxiliary_projectile(
                        projectile_slot,
                        target,
                        terrain,
                    );
                    return;
                }
            }
        }
        level2_castle_auxiliary_settle(&mut self.castle_children[child_index].position, terrain);
    }

    fn step_castle_auxiliary_slot(
        &mut self,
        child_index: usize,
        entities: &mut [EntitySnapshot],
        membership: &Level2CastleAuxiliaryMembership,
        terrain: &SessionTerrain,
    ) -> bool {
        let source_slot = self.castle_children[child_index].source_entity_index;
        match self.castle_children[child_index].action {
            121 => {
                self.step_castle_auxiliary_action121(child_index, entities, membership, terrain);
            }
            122 => self.step_castle_auxiliary_action122(child_index, entities, terrain),
            124 => {
                let mut linked_slot = self.castle_children[child_index].chain_next_slot;
                let mut visited = BTreeSet::new();
                while linked_slot != 0 && visited.insert(linked_slot) {
                    let Some(linked_index) = self
                        .castle_children
                        .iter()
                        .position(|child| child.active && child.source_entity_index == linked_slot)
                    else {
                        break;
                    };
                    let next_slot = self.castle_children[linked_index].chain_next_slot;
                    self.castle_children[linked_index].action = 125;
                    if self.castle_children[linked_index].killer_entity_index != 0 {
                        self.castle_children[child_index].killer_entity_index =
                            self.castle_children[linked_index].killer_entity_index;
                    }
                    linked_slot = next_slot;
                }
                self.castle_children[child_index].action = 125;
            }
            125 => {
                if self.castle_children[child_index].source_byte_3e & 7 == 0 {
                    let child = self.castle_children[child_index];
                    let mut replacement_slot = None;
                    if child.control_flags & 0x0010_0000 == 0 {
                        replacement_slot = self.spawn_death_burst_from_death_with_id(
                            child.source_entity_index,
                            child.id_entity_index,
                            child.position,
                        );
                    }
                    if replacement_slot == Some(source_slot) {
                        if let Some(burst) = self
                            .death_bursts
                            .iter_mut()
                            .find(|burst| burst.source_entity_index == source_slot)
                        {
                            // KillEntity continues through its aliased source
                            // pointer after NewEvent overwrites the row.
                            burst.id_entity_index = source_slot;
                            burst.position = Position::default();
                            burst.control_flags |= ENTITY_DRAW_DISABLED_BIT;
                            burst.active = false;
                        }
                        if let Some(Level2EconomyEvent::DeathBurstSpawned {
                            id_entity_index, ..
                        }) = self.events.iter_mut().rev().find(|event| {
                            matches!(
                                event,
                                Level2EconomyEvent::DeathBurstSpawned {
                                    source_entity_index,
                                    ..
                                } if *source_entity_index == source_slot
                            )
                        }) {
                            *id_entity_index = source_slot;
                        }
                        self.map_chains
                            .sync_entity(RenderableMapSeed {
                                entity_index: source_slot,
                                active: true,
                                x: 0,
                                y: 0,
                            })
                            .expect("self-replaced death burst stays map-linked until prepass");
                    } else if let Some(child) = self
                        .castle_children
                        .iter_mut()
                        .find(|child| child.source_entity_index == source_slot)
                    {
                        child.control_flags |= ENTITY_DRAW_DISABLED_BIT;
                    }
                }
            }
            _ => return false,
        }
        self.finish_castle_auxiliary_source_visit(source_slot)
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
        if child.source_class == 5 && child.model == 15 {
            return self.step_castle_auxiliary_slot(
                child_index,
                entities,
                auxiliary_membership,
                terrain,
            );
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
        let hit = self.castle.is_some_and(|castle| {
            castle.active
                && castle.source_class == 3
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
        if let Some(castle) = self.castle.as_mut() {
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
        let Some(parent) = self.castle.filter(|castle| {
            castle.active && castle.source_entity_index == child.parent_entity_index
        }) else {
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
        let Some(parent) = self.castle.filter(|castle| {
            castle.active && castle.source_entity_index == child.parent_entity_index
        }) else {
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
        if let Some(parent) = self
            .castle
            .as_mut()
            .filter(|castle| castle.source_entity_index == parent_slot)
        {
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
    pub fn step_mana_sphere_slot(&mut self, sphere_slot: u16, terrain: &SessionTerrain) -> bool {
        let Some(sphere_index) = self
            .spheres
            .iter()
            .position(|sphere| sphere.active && sphere.source_entity_index == sphere_slot)
        else {
            return false;
        };
        // `TransformArcherToMana_35940` consumes the model-12 mailbox before
        // movement. A tier-0 pulse transfers only when the raw protected bit
        // is clear; both owner projections are authoritative consumers in
        // the Level 2 economy, so they must change together.
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
                if sphere.owner_entity_index != pending_owner && (payload != 0 || !protected) {
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
            sphere.phase = sphere.phase.wrapping_add(1);
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

        if grounded {
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
        let castle_snapshot = self.castle.filter(|castle| {
            castle.active && child_snapshot.target_sphere_slot == castle.source_entity_index
        });
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
                    if let Some(target) = self
                        .castle
                        .as_mut()
                        .filter(|target| target.source_entity_index == castle.source_entity_index)
                    {
                        target.mana = target.mana.wrapping_add(child_snapshot.mana);
                    }
                }
            } else {
                crate::move_entity(&mut predicted, yaw, 0, child_snapshot.speed);
            }
        } else if let Some(sphere_index) = self.spheres.iter().position(|sphere| {
            sphere.active && sphere.source_entity_index == child_snapshot.target_sphere_slot
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
                predicted.z = predicted.z.wrapping_add(LEVEL2_CASTLE_DEFENDER_CAVE_STEP);
            }
            if i32::from(predicted.z)
                <= i32::from(ground) + i32::from(LEVEL2_CASTLE_DEFENDER_CAVE_FLOOR_CLEARANCE)
            {
                predicted.z = ground.saturating_add(LEVEL2_CASTLE_DEFENDER_CAVE_FLOOR_CLEARANCE);
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
            // `AddBallon_60AB0` uses the common non-Cave profile with the
            // default str_160 words (c=0, a=0, e=-4) after targeting.
            if predicted.z > ground {
                predicted.z = predicted.z.wrapping_sub(4);
            }
            if predicted.z <= ground {
                predicted.z = ground;
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
            if let Some(castle) = self.castle.filter(|castle| castle.active) {
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
                        self.castle.as_mut().expect("Castle remains installed").mana =
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
                carrier.pending_damage = 0;
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
        let owner = if target_owner == 0 {
            source_owner
        } else if source_owner == 0 {
            target_owner
        } else if self.owner_class.get(&target_owner) == Some(&10)
            && self.owner_class.get(&source_owner) == Some(&10)
        {
            target_owner
        } else if self.owner_class.get(&target_owner) == Some(&10)
            && self.owner_class.get(&source_owner) != Some(&10)
        {
            source_owner
        } else if self.owner_class.get(&source_owner) == Some(&10)
            && self.owner_class.get(&target_owner) != Some(&10)
        {
            target_owner
        } else if self
            .owner_max_mana
            .get(&target_owner)
            .copied()
            .unwrap_or_default()
            > self
                .owner_max_mana
                .get(&source_owner)
                .copied()
                .unwrap_or_default()
        {
            target_owner
        } else {
            source_owner
        };
        let source_mana = self.spheres[source_index].mana;
        let target_id = self.spheres[target_index].id;
        let source_id = self.spheres[source_index].id;
        let source_entity_rng = self.spheres[source_index].entity_rng;
        let source_position = self.spheres[source_index].position;
        self.spheres[target_index].mana = self.spheres[target_index].mana.wrapping_add(source_mana);
        self.spheres[target_index].owner_entity_index = owner;
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
        let (target_position, target_anchor) =
            level2_castle_root_positions(terrain, &footprint, payload.position);
        let target_castle_slot = self
            .castle
            .filter(|castle| {
                castle.active && self.player_castle_entity_index == castle.source_entity_index
            })
            .map_or(0, |castle| castle.source_entity_index);
        let mut axis_position = if target_castle_slot == 0 {
            payload.position
        } else {
            self.castle
                .filter(|castle| castle.source_entity_index == target_castle_slot)
                .map_or(payload.position, |castle| castle.position)
        };
        let initial_yaw = payload.yaw;
        if target_castle_slot == 0 {
            crate::move_entity(&mut axis_position, initial_yaw, 0, 4_096);
            axis_position.z = terrain.ground_height(axis_position);
        }
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
            if self.castle_projectile_site_blocked(projectile, entities, terrain) {
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
            self.castle
                .filter(|castle| {
                    castle.active && castle.source_entity_index == projectile.source_word_96
                })
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
            let castle_hit = self.castle.filter(|castle| {
                projectile.source_word_96 != 0
                    && castle.active
                    && castle.source_entity_index == projectile.source_word_96
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
                && self.castle_projectile_site_blocked(moved, entities, terrain);
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
        let root = self
            .castle
            .filter(|castle| castle.active && castle.source_entity_index == target_castle_slot)?;
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
    fn castle_projectile_site_blocked(
        &self,
        projectile: Level2CastleProjectile,
        entities: &[EntitySnapshot],
        terrain: &SessionTerrain,
    ) -> bool {
        let start_x = (projectile.position.x >> 8) as u8;
        let start_y = (projectile.position.y >> 8) as u8;
        let nearby_castle = self.castle.is_some_and(|castle| {
            castle.active
                && castle.source_class == 3
                && castle.source_model == 2
                && level2_castle_nearby_projectile(castle, projectile.position)
        }) || entities.iter().any(|entity| {
            entity.active
                && entity.life >= 0
                && entity.class == 3
                && entity.model == 2
                && level2_entity_nearby_projectile(entity, projectile.position)
        });
        let nearby_model45 = entities.iter().any(|entity| {
            entity.active
                && entity.class == 10
                && entity.model == 45
                && level2_entity_nearby_projectile(entity, projectile.position)
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
        let pending = self
            .castle
            .as_ref()
            .is_some_and(|castle| castle.upgrade_holder_clear_pending);
        if pending {
            if let Some(castle) = self.castle.as_mut() {
                castle.upgrade_holder_clear_pending = false;
            }
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

    fn install_castle_root(
        &mut self,
        source_entity_index: u16,
        position: Position,
        anchor_position: Position,
        owner_entity_index: u16,
        source_constructor: bool,
        emit_built_event: bool,
    ) -> Option<u16> {
        self.castle = Some(Level2CastleState {
            source_entity_index,
            // The source Castle root stores the owning player's logical
            // entity id in id_0x1A; the arena slot remains separate.
            source_id_entity_index: owner_entity_index,
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
        self.player_castle_entity_index = if source_constructor {
            0
        } else {
            source_entity_index
        };
        self.player_castle_stage = if source_constructor { 0 } else { 1 };
        self.player_castle_aux_flags = 0;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .expect("Castle root source slot fits the map chain");
        if emit_built_event {
            self.events.push(Level2EconomyEvent::CastleBuilt {
                source_entity_index,
                owner_entity_index,
            });
        }
        Some(source_entity_index)
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
        if self.castle.is_some_and(|castle| castle.active) {
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
        if self.castle.is_some_and(|castle| castle.active) {
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
        )
    }

    /// Set the source Castle anchor (`axis_0x9A_154x`) used by child creation
    /// and the action-4 model-39 chain scan. The root position remains the
    /// terrain-synchronised entity position.
    pub fn set_castle_anchor(&mut self, castle_slot: u16, anchor: Position) -> bool {
        let Some(castle) = self
            .castle
            .as_mut()
            .filter(|castle| castle.active && castle.source_entity_index == castle_slot)
        else {
            return false;
        };
        castle.anchor_position = anchor;
        true
    }

    /// Re-enter the source Castle construction action for a reset root. This
    /// is the bounded `BeginOfCastleCreation_5FA70` entry; child allocation is
    /// committed only by the following `step_castle_action` call.
    pub fn start_castle_construction(&mut self, castle_slot: u16) -> bool {
        let Some(castle) = self
            .castle
            .as_mut()
            .filter(|castle| castle.active && castle.source_entity_index == castle_slot)
        else {
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
        self.player_castle_stage = 0;
        self.player_castle_aux_flags = 0;
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
        let Some(castle) = self.castle.filter(|castle| {
            castle.active
                && castle.source_entity_index == castle_slot
                && castle.action == 5
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
        if let Some(castle) = self
            .castle
            .as_mut()
            .filter(|castle| castle.source_entity_index == castle_slot)
        {
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
        let Some(castle) = self
            .castle
            .as_mut()
            .filter(|castle| castle.active && castle.source_entity_index == castle_slot)
        else {
            return false;
        };
        if castle.action != 5 || castle.build_state != 4 {
            return false;
        }
        castle.build_state = 5;
        true
    }

    fn advance_castle_phase(&mut self, castle_slot: u16) {
        if let Some(castle) = self
            .castle
            .as_mut()
            .filter(|castle| castle.source_entity_index == castle_slot)
        {
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
            .castle
            .filter(|castle| castle.source_entity_index == castle_slot)
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
        if let Some(castle) = self
            .castle
            .as_mut()
            .filter(|castle| castle.source_entity_index == castle_slot)
        {
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
        let Some(castle) = self
            .castle
            .filter(|castle| castle.source_entity_index == castle_slot)
        else {
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
                entity_rng: sphere_rng,
                position: sphere_position,
                velocity_x: 0,
                velocity_y: 0,
                vertical_velocity,
                yaw: 0,
                mana: sphere_mana,
                owner_entity_index: castle.owner_entity_index,
                // `sub_5FD00` writes the Castle root's raw id relation, not
                // the separate presentation owner field.  The defender's
                // `sub_5F810` gate compares against this same id.
                player_entity_index: castle.source_id_entity_index,
                incoming_owner_entity_index: 0,
                incoming_possession_payload: 0,
                attraction_source_entity_index: 0,
                attraction_step: 0,
                attraction_distance_high: 0,
                assigned_carrier_id: 0,
                assigned_defender_slot: 0,
                control_flags: 0,
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
            if let Some(castle) = self.castle.as_mut() {
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
        let Some(snapshot) = self
            .castle
            .filter(|castle| castle.active && castle.source_entity_index == castle_slot)
        else {
            return;
        };
        if snapshot.level == 0 {
            return;
        }
        let reduced_capacity = snapshot
            .mana_capacity
            .saturating_sub(snapshot.mana_capacity / 10);
        if let Some(castle) = self.castle.as_mut() {
            castle.mana_capacity = reduced_capacity;
        }
        self.release_castle_excess_mana(castle_slot, terrain, reset_free_slots);
        if let Some(castle) = self.castle.as_mut() {
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
            level2_castle_controller_selection(level, &self.castle_controller_coefficients)
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
        if let Some(castle) = self.castle.as_mut() {
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
        let Some(snapshot) = self
            .castle
            .filter(|castle| castle.active && castle.source_entity_index == castle_slot)
        else {
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
                    visual_index: 0,
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
        if let Some(castle) = self
            .castle
            .as_mut()
            .filter(|castle| castle.source_entity_index == castle_slot)
        {
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
        // compress the loop after a dead indexed holder is cleared: with
        // v21=2, a live source slot 1 still tests byte_0x3E % 2 and uses
        // source index 1 for the modulo-3 exclusion relations.
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
            } else if child.source_byte_3e % u8::try_from(defender_target_count.max(1)).unwrap_or(1)
                == 0
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
        if let Some(castle) = self
            .castle
            .as_mut()
            .filter(|castle| castle.source_entity_index == castle_slot)
        {
            castle.defender_total_mana = defender_total_mana;
            castle.defender_total_max_mana = defender_total_max_mana;
            castle.defender_target_spheres = defender_targets;
        }

        // The source then walks `array_0x5C_92`, first decrementing the root
        // word-0x2C cooldown once per call.  Existing relations are valid
        // only for class 5/model 15 rows whose action is not the delete
        // sentinel 125; an invalid relation clears only the root slot and
        // restarts the 16-frame creation cooldown.
        if let Some(castle) = self
            .castle
            .as_mut()
            .filter(|castle| castle.source_entity_index == castle_slot)
        {
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
            .castle
            .filter(|castle| castle.source_entity_index == castle_slot)
            .map(|castle| castle.auxiliary_cooldown)
            .unwrap_or_default();
        for index in 0..auxiliary_target {
            let slot = auxiliary_slots[index];
            if slot == 0 {
                if cooldown == 0 {
                    let child_position = Position {
                        x: snapshot.position.x.wrapping_add(128),
                        y: snapshot.position.y.wrapping_add(640),
                        z: 0,
                    };
                    let child_position = Position {
                        z: terrain.ground_height(child_position),
                        ..child_position
                    };
                    if let Some(slot) = self.allocate_slot() {
                        // `sub_4C1E0` consumes the shared per-model counter,
                        // then derives byte 0x39 from descriptor 83 before
                        // `sub_5FF50` overwrites the live yaw/roll/position.
                        let source_byte_3e = self.castle_auxiliary_constructor_phase;
                        self.castle_auxiliary_constructor_phase =
                            self.castle_auxiliary_constructor_phase.wrapping_add(1);
                        let source_byte_39 = LEVEL2_CASTLE_AUXILIARY_CADENCE
                            .wrapping_sub(source_byte_3e % LEVEL2_CASTLE_AUXILIARY_CADENCE)
                            .wrapping_add(4);
                        let child = Level2CastleChild {
                            source_entity_index: slot,
                            entity_rng: self.new_event_rng(slot),
                            id_entity_index: snapshot.source_id_entity_index,
                            source_class: 5,
                            // `sub_5FF50` owns this row through
                            // array_0x5C_92, not parentId.
                            parent_entity_index: 0,
                            owner_entity_index: snapshot.owner_entity_index,
                            player_entity_index: snapshot.source_id_entity_index,
                            position: child_position,
                            axis_position: Position::default(),
                            model: 15,
                            action: 121,
                            mana: 0,
                            max_mana: 0,
                            life: LEVEL2_CASTLE_AUXILIARY_MAX_LIFE,
                            max_life: LEVEL2_CASTLE_AUXILIARY_MAX_LIFE,
                            speed: LEVEL2_CASTLE_AUXILIARY_MINIMUM_SPEED,
                            target_sphere_slot: 0,
                            pending_damage: 0,
                            pending_attacker_entity_index: 0,
                            damage_attacker_entity_index: 0,
                            killer_entity_index: 0,
                            source_byte_3e,
                            source_byte_39,
                            source_byte_3b: 0,
                            cave_collision_cooldown: 0,
                            source_dword_10: i32::from(slot % 100),
                            source_word_2e: 0,
                            construction_target_height: 0,
                            source_subspell_index: 500,
                            yaw: 512,
                            roll: 512,
                            visual_index: 0,
                            // `sub_4C1E0` sets byte[2] & 2, making this
                            // class-5/model-15 row eligible for the
                            // `NewEvent_4A050` replaceable fallback.
                            control_flags: SOURCE_REPLACEABLE_FLAG | 0x8,
                            chain_previous_slot: 0,
                            chain_next_slot: 0,
                            source_byte_43: 0,
                            source_word_4a: 0,
                            active: true,
                        };
                        self.sync_castle_child_map(child);
                        self.castle_children.push(child);
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
            let valid = self.castle_children.iter().any(|child| {
                child.active
                    && child.source_entity_index == slot
                    && child.source_class == 5
                    && child.model == 15
                    && child.action != 125
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
        if let Some(castle) = self
            .castle
            .as_mut()
            .filter(|castle| castle.source_entity_index == castle_slot)
        {
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
        let Some(snapshot) = self
            .castle
            .filter(|castle| castle.active && castle.source_entity_index == castle_slot)
        else {
            return false;
        };
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
                let stage_coefficient = self
                    .castle_stage_coefficients
                    .get(usize::from(next_level))
                    .copied()
                    .unwrap_or(256);
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
                    &self.castle_controller_coefficients,
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
                let castle = self.castle.as_mut().expect("Castle remains installed");
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
                self.player_castle_entity_index = castle.source_entity_index;
                self.player_castle_stage = castle.level;
                self.player_castle_aux_flags = 0;
                self.events
                    .push(Level2EconomyEvent::CastleConstructionSound {
                        source_entity_index: castle.source_entity_index,
                        sound_id: 10,
                    });
                self.advance_castle_phase(castle_slot);
                true
            }
            5 if snapshot.build_state == 2 => {
                let castle = self.castle.as_mut().expect("Castle remains installed");
                castle.action = 4;
                castle.build_state = 0;
                castle.shift_yaw = -8_192;
                castle.shift_fov = 0x4000;
                self.advance_castle_phase(castle_slot);
                true
            }
            5 if snapshot.build_state == 1 => {
                let castle = self.castle.as_mut().expect("Castle remains installed");
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
                self.castle
                    .as_mut()
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
                let castle = self.castle.as_mut().expect("Castle remains installed");
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
                self.castle
                    .as_mut()
                    .expect("Castle remains installed")
                    .position
                    .z = terrain.ground_height(snapshot.position);
                self.advance_castle_phase(castle_slot);
                true
            }
            4 if snapshot.source_word_80 == snapshot.source_id_entity_index
                && snapshot.source_word_7c == 10 =>
            {
                let castle = self.castle.as_mut().expect("Castle remains installed");
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
                let castle = self.castle.as_mut().expect("Castle remains installed");
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
                self.castle
                    .as_mut()
                    .expect("Castle remains installed")
                    .action = 6;
                self.advance_castle_phase(castle_slot);
                true
            }
            4 if snapshot.control_flags & 0x40 != 0 => {
                let castle = self.castle.as_mut().expect("Castle remains installed");
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
                    if self.castle.is_some_and(|castle| castle.life < 0) {
                        self.castle
                            .as_mut()
                            .expect("Castle remains installed")
                            .action = 6;
                        self.advance_castle_phase(castle_slot);
                        return true;
                    }
                    if self
                        .castle
                        .is_some_and(|castle| castle.control_flags & 0x40 != 0)
                    {
                        let castle = self.castle.as_mut().expect("Castle remains installed");
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
                if let Some(castle) = self.castle.as_mut() {
                    castle.position.z = terrain.ground_height(castle.position);
                }
                self.release_castle_excess_mana(castle_slot, terrain, reset_free_slots);
                self.maintain_castle_defenders(castle_slot, terrain);
                let Some(castle) = self.castle else {
                    self.advance_castle_phase(castle_slot);
                    return false;
                };
                if castle.mana >= castle.mana_capacity {
                    self.advance_castle_phase(castle_slot);
                    return true;
                }
                let Some(sphere_index) = self.spheres.iter().position(|sphere| {
                    sphere.active
                        && sphere.owner_entity_index == castle.owner_entity_index
                        && level2_castle_sphere_overlap(castle.position, sphere)
                }) else {
                    self.advance_castle_phase(castle_slot);
                    return true;
                };
                let sphere = self.spheres[sphere_index];
                let castle = self.castle.as_mut().expect("Castle remains installed");
                castle.mana = castle.mana.wrapping_add(sphere.mana);
                self.spheres[sphere_index].pending_delete = true;
                self.spheres[sphere_index].control_flags |= ENTITY_DRAW_DISABLED_BIT;
                self.events.push(Level2EconomyEvent::ManaSphereCollected {
                    sphere_id: sphere.id,
                    carrier_id: u32::from(castle.source_entity_index),
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
                    if let Some(castle) = self
                        .castle
                        .as_mut()
                        .filter(|castle| castle.source_entity_index == castle_slot)
                    {
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
                let stage_coefficient = self
                    .castle_stage_coefficients
                    .get(usize::from(next_level))
                    .copied()
                    .unwrap_or(256);
                let stage_life =
                    i32::try_from((i64::from(base_life) * i64::from(stage_coefficient)) >> 8)
                        .unwrap_or(if base_life >= 0 { i32::MAX } else { i32::MIN });
                let (shift_pitch, shift_roll) = level2_castle_shift_axes(next_level);
                let recovered_life = if next_level == 0 {
                    0
                } else {
                    let deficit = if snapshot.life < 0 {
                        snapshot.life.saturating_neg().min(stage_life / 2)
                    } else {
                        0
                    };
                    stage_life.saturating_sub(deficit)
                };
                let remaining_mana = self.castle.map(|castle| castle.mana).unwrap_or_default();
                let castle = self.castle.as_mut().expect("Castle remains installed");
                castle.action = 4;
                castle.build_state = 0;
                castle.level = next_level;
                castle.life = recovered_life;
                castle.max_life = stage_life;
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
                castle.source_word_2e = 0;
                castle.source_word_30 = 0;
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
                castle.timer = 5;
                self.events.push(Level2EconomyEvent::CastleStageDestroyed {
                    source_entity_index: castle.source_entity_index,
                    previous_level: snapshot.level,
                    next_level,
                });
                if castle.level == 0 {
                    castle.active = false;
                    self.player_castle_entity_index = 0;
                    self.player_castle_stage = 0;
                    self.player_castle_aux_flags = 0;
                    if self.player_model == 1 && self.player_terrain_flags & 4 != 0 {
                        self.player_spell_slot_2 = 0;
                        self.player_spell_holder_action = 0;
                    }
                    self.events.push(Level2EconomyEvent::CastleDestroyed {
                        source_entity_index: castle.source_entity_index,
                        killer_entity_index: castle.killer_entity_index,
                    });
                } else {
                    self.player_castle_stage = castle.level;
                    self.player_castle_aux_flags = 0;
                }
                // `sub_5F890(a1, 1)` copies the holder's own max counter;
                // it is not derived from the Castle stage number.  The final
                // non-player path calls `sub_5F890(a1, 0)` and clears it,
                // while a model-1 player on terrain flag 4 disables the
                // holder relation entirely.
                if self.player_spell_slot_2 != 0 {
                    if next_level > 0 {
                        self.player_spell_holder_action =
                            self.player_spell_holder_max_action.saturating_sub(1);
                    } else {
                        self.player_spell_holder_action = 0;
                    }
                }
                if next_level > 0 {
                    let downgraded = self.castle.expect("Castle remains installed");
                    self.spawn_castle_controller_chain(downgraded, next_level, terrain);
                }
                // `sub_5FCA0` performs a second overflow release and then
                // runs `sub_5FF50` after `sub_605E0` has rebuilt the lower
                // stage. Allocation failure here must not roll back the
                // already-committed stage transition.
                self.release_castle_excess_mana(castle_slot, terrain, reset_free_slots);
                self.maintain_castle_defenders(castle_slot, terrain);
                if let Some(castle) = self
                    .castle
                    .as_mut()
                    .filter(|castle| castle.source_entity_index == castle_slot)
                {
                    // `sub_5FCA0` clears word-0x2E and leaves word-0x30 at
                    // the post-downgrade timer sentinel 5.
                    castle.source_word_2e = 0;
                    castle.source_word_30 = 5;
                }
                if let Some(castle) = self.castle.filter(|castle| !castle.active) {
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
        let Some(castle) = self.castle.as_mut().filter(|castle| castle.active) else {
            return false;
        };
        let Some(carrier) = self
            .carriers
            .iter_mut()
            .find(|carrier| carrier.active && carrier.source_entity_index == carrier_slot)
        else {
            return false;
        };
        if carrier.owner_entity_index != castle.owner_entity_index {
            return false;
        }
        carrier.target_sphere_id = u32::from(castle.source_entity_index);
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
        let Some(castle) = self.castle.filter(|castle| castle.active) else {
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
        let castle_state = self.castle.as_mut().expect("Castle remains installed");
        castle_state.mana = castle_state.mana.wrapping_add(mana);
        carrier.carried_mana = 0;
        carrier.target_sphere_id = 0;
        carrier.returning_to_castle = false;
        carrier.life = carrier.max_life;
        self.events.push(Level2EconomyEvent::ManaCarrierArrived {
            carrier_id: carrier.id,
            castle_id: castle.source_entity_index,
            mana,
        });
        true
    }

    pub fn submit_damage(&mut self, target_slot: u16, damage: i32, attacker: u16) -> bool {
        if damage <= 0 || attacker == 0 {
            return false;
        }
        if let Some(auxiliary) = self.castle_children.iter_mut().find(|child| {
            child.active
                && child.life >= 0
                && child.source_entity_index == target_slot
                && child.source_class == 5
                && child.model == 15
                && matches!(child.action, 121 | 122)
        }) {
            auxiliary.pending_damage = if auxiliary.pending_attacker_entity_index == 0 {
                damage
            } else {
                auxiliary.pending_damage.wrapping_add(damage)
            };
            auxiliary.pending_attacker_entity_index = attacker;
            return true;
        }
        if let Some(carrier) = self.carriers.iter_mut().find(|carrier| {
            carrier.active && carrier.life >= 0 && carrier.source_entity_index == target_slot
        }) {
            carrier.pending_damage = if carrier.pending_attacker_entity_index == 0 {
                damage
            } else {
                carrier.pending_damage.wrapping_add(damage)
            };
            carrier.pending_attacker_entity_index = attacker;
            return true;
        }
        if let Some(castle) = self.castle.as_mut().filter(|castle| {
            castle.active && castle.life >= 0 && castle.source_entity_index == target_slot
        }) {
            castle.pending_damage = if castle.pending_attacker_entity_index == 0 {
                damage
            } else {
                castle.pending_damage.wrapping_add(damage)
            };
            castle.pending_attacker_entity_index = attacker;
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
        let castle = self.castle.as_mut()?;
        if !castle.active || castle.source_entity_index != target_slot {
            return None;
        }
        let damage = castle.pending_damage;
        let attacker = castle.pending_attacker_entity_index;
        if attacker == 0 {
            return None;
        }
        castle.life = castle.life.wrapping_sub(damage);
        castle.pending_damage = 0;
        castle.pending_attacker_entity_index = 0;
        let lethal = castle.life < 0;
        if lethal {
            castle.killer_entity_index = attacker;
        } else {
            // sub_609E0 clears the damage mailbox, marks the owner's linked
            // player record, and requests state-0 reconstruction when the
            // attacker mailbox points back at this Castle root.
            self.player_castle_aux_flags = 4;
            if attacker == castle.source_id_entity_index && castle.level < 7 {
                castle.control_flags |= 0x40;
            }
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
