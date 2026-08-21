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
    /// an already occupied mailbox is rejected so a one-shot projectile cannot
    /// duplicate its hit before the Bee callback consumes it.
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
                && bee.pending_damage == 0
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
    pub(super) fn step_fireball_projectile_slot(&mut self, source_slot: u16) -> Level2FireballStep {
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
                                    && bee.pending_damage == 0
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
                let ground = self.terrain.ground_height(projectile.position);
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
    pub fn step_possession_projectile_slot(&mut self, source_slot: u16) -> bool {
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
        let ground_height = self.terrain.ground_height(moved.position);
        if ground_height > moved.position.z {
            moved.position.z = ground_height;
        }
        let cave_limit = self
            .terrain
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
    ) -> Option<u32> {
        self.spawn_mana_spheres_from_death(
            source_entity_index,
            position,
            source_yaw,
            source_rng_after_death,
            mana,
            owner_entity_index,
            false,
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
            let ground = self.terrain.ground_height(position);
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

    /// Advance one dynamic Castle child through its source callback or the
    /// terminal no-callback branch. Model 79/action 0x56 is the live
    /// `sub_3AF00` Castle-defend callback; model 42/action 44 is the
    /// BUILD-backed 37BC0 callback; model 41/action 43 remains the separate
    /// MSPRD-backed 377F0 callback. Other terminal rows retain source
    /// disable/reclaim.
    pub fn step_castle_child_terminal_slot(&mut self, source_slot: u16) -> bool {
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
            return self.step_castle_controller_slot(child_index);
        }
        if child.source_class == 10 && child.model == 43 && child.action == 45 {
            return self.step_castle_upgrade_product(child_index);
        }
        if child.source_class == 10 && child.model == 42 && child.action == 44 {
            let live = self.step_castle_build_child(child_index);
            // `UpdateEntities_57730` performs the ordinary callback epilogue
            // after 37BC0 returns, including the byte_0x3E_62 increment.
            if let Some(child) = self.castle_children.get_mut(child_index) {
                child.source_byte_3e = child.source_byte_3e.wrapping_add(1);
            }
            return live;
        }
        if child.source_class == 10 && child.model == 41 && child.action == 43 {
            if level2_castle_model41_msprd_rect(child.cave_collision_cooldown).is_some() {
                let live = self.step_castle_msprd_child(child_index);
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
    fn step_castle_controller_slot(&mut self, child_index: usize) -> bool {
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

            let ground = self.terrain.ground_height(child.position);
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

    fn step_castle_build_child(&mut self, child_index: usize) -> bool {
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
                return self.complete_castle_construction(parent.source_entity_index);
            }
            return true;
        }

        clock = clock.wrapping_sub(1);
        if clock == 0 {
            clock = if child.source_byte_3b != 0 { -25 } else { -1 };
        } else if parent.source_word_30 == 0 {
            let remaining = i16::try_from(clock).expect("37BC0 BUILD timer fits i16");
            self.terrain
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
    fn step_castle_msprd_child(&mut self, child_index: usize) -> bool {
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
                &self.terrain.planes,
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
                        self.terrain.planes.ground_heights[index] =
                            self.terrain.planes.ground_heights[index].wrapping_add(step as u8);
                        if clock == 1 {
                            let mut angle = self.terrain.planes.angles[index];
                            if angle & 0x80 != 0 {
                                angle &= !0x80;
                                if self.terrain.map_type == MapType::Day {
                                    angle |= 0x08;
                                }
                                self.terrain.planes.angles[index] = angle;
                            }
                        }
                    }
                }
            } else if clock == -1 && self.terrain.map_type == MapType::Day {
                for row in 0..height {
                    for column in 0..width {
                        let index =
                            terrain_index(start_x.wrapping_add(column), start_y.wrapping_add(row));
                        let angle = &mut self.terrain.planes.angles[index];
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
                self.terrain.mutation_sequence = self.terrain.mutation_sequence.wrapping_add(1);
            }
            return true;
        }

        self.complete_castle_msprd_child(parent.source_entity_index, child, width, height);
        true
    }

    fn complete_castle_msprd_child(
        &mut self,
        parent_slot: u16,
        child: Level2CastleChild,
        width: u8,
        height: u8,
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
                    &mut self.terrain.planes,
                    left.wrapping_add(offset),
                    top.wrapping_add(row),
                    self.terrain.map_type,
                );
                smooth_building_height(
                    &mut self.terrain.planes,
                    center_x.wrapping_add(width / 2).wrapping_add(offset),
                    top.wrapping_add(row),
                    self.terrain.map_type,
                );
            }
        }
        for column in 0..width.saturating_add(6) {
            for offset in 0..4 {
                smooth_building_height(
                    &mut self.terrain.planes,
                    left.wrapping_add(column),
                    top.wrapping_sub(3).wrapping_add(offset),
                    self.terrain.map_type,
                );
                smooth_building_height(
                    &mut self.terrain.planes,
                    left.wrapping_add(column),
                    top.wrapping_add(height.saturating_sub(1))
                        .wrapping_add(offset),
                    self.terrain.map_type,
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
    pub fn step_mana_sphere_slot(&mut self, sphere_slot: u16) -> bool {
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
            let ground = self.terrain.ground_height(sphere.position);
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
                    i32::from(self.terrain.ground_height(Position {
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
    pub fn step_castle_defender_slot(&mut self, source_slot: u16) -> bool {
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
                let ground = self.terrain.ground_height(predicted);
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
            let attraction_limit = if self.terrain.map_type == MapType::Cave {
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
        let ground = self.terrain.ground_height(predicted);
        if self.terrain.map_type == MapType::Cave {
            // Pinned remc2 `sub_60D50` (EventsFunctions.cpp:61920-61976):
            // a Cave holder is blocked by the map angle bit 8 or by the
            // strict `sub_11E70` ceiling test.  The latter is
            // `fov + ground + str_D7BD6[68].word_0xc (512) > ceiling`.
            let tile = crate::Tile {
                x: predicted.x.to_be_bytes()[0],
                y: predicted.y.to_be_bytes()[0],
            };
            let angle_blocked = self.terrain.angle_flags(tile) & 8 != 0;
            let ceiling_blocked = self
                .terrain
                .ceiling_height(predicted)
                .is_some_and(|ceiling| {
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
                if let Some(ceiling) = self.terrain.ceiling_height(predicted) {
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
            sphere_state.active = false;
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: sphere.source_entity_index,
                    active: false,
                    x: sphere.position.x,
                    y: sphere.position.y,
                })
                .expect("Level 2 defender sphere source slot fits the map chain");
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
    pub fn step_mana_carrier_slot(&mut self, carrier_slot: u16) -> bool {
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
                    let ground = self.terrain.ground_height(position);
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
                            self.spheres[sphere_index].active = false;
                            self.map_chains
                                .sync_entity(RenderableMapSeed {
                                    entity_index: sphere.source_entity_index,
                                    active: false,
                                    x: sphere.position.x,
                                    y: sphere.position.y,
                                })
                                .expect("Level 2 sphere source slot fits the map chain");
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
            let ground = self.terrain.ground_height(position);
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
        sphere_state.active = false;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: sphere.source_entity_index,
                active: false,
                x: sphere.position.x,
                y: sphere.position.y,
            })
            .expect("Level 2 sphere source slot fits the map chain");
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
        self.carriers.push(carrier);
        self.refresh_owner_mana_rows();
        self.events.push(Level2EconomyEvent::ManaCarrierSpawned {
            source_entity_index,
            owner_entity_index,
        });
        Some(source_entity_index)
    }

    pub fn build_castle(&mut self, position: Position, owner_entity_index: u16) -> Option<u16> {
        let mut root_position = position;
        root_position.z = self.terrain.ground_height(root_position);
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
    ) -> Option<u16> {
        self.build_castle_with_footprint_mode(
            projectile_position,
            owner_entity_index,
            footprint,
            true,
        )
    }

    fn build_castle_with_footprint_mode(
        &mut self,
        projectile_position: Position,
        owner_entity_index: u16,
        footprint: &CastleBuildFootprint,
        emit_built_event: bool,
    ) -> Option<u16> {
        if footprint.index != 1 {
            return None;
        }
        let (root_position, construction_anchor) =
            level2_castle_root_positions(&self.terrain, footprint, projectile_position);
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
    ) -> Option<u16> {
        let footprint = self.castle_stage_footprints.get(&1)?.clone();
        let (target_position, target_anchor) =
            level2_castle_root_positions(&self.terrain, &footprint, payload.position);
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
            axis_position.z = self.terrain.ground_height(axis_position);
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
            if self.castle_projectile_site_blocked(projectile, entities) {
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
                let ground = self.terrain.ground_height(moved.position);
                let moved_z = moved.position.z;
                if ground > moved_z {
                    moved.position.z = ground;
                    terrain_hit = true;
                }
                let moved_z = moved.position.z;
                if self.terrain.map_type == MapType::Cave
                    && let Some(ceiling) = self.terrain.ceiling_height(moved.position)
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
                && self.castle_projectile_site_blocked(moved, entities);
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
                level2_castle_root_positions(&self.terrain, &footprint, landing_position);
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
            source_byte_3e: 0,
            source_byte_3b: 0,
            cave_collision_cooldown: 0,
            source_dword_10: 0,
            source_word_2e: 0,
            construction_target_height: 0,
            source_subspell_index: 64_000,
            yaw: 512,
            roll: 512,
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
                let flags = self.terrain.angle_flags(crate::Tile {
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
        self.baseline_castle_stage_footprints
            .insert(stage, footprint.clone());
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
    pub fn complete_castle_construction(&mut self, castle_slot: u16) -> bool {
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
        self.terrain
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
    fn release_castle_excess_mana(&mut self, castle_slot: u16) {
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
            self.rebuild_free_arena();
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
            let ground = self.terrain.ground_height(castle.position);
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
    fn prepare_castle_stage_destruction(&mut self, castle_slot: u16) {
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
        self.release_castle_excess_mana(castle_slot);
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
    fn spawn_castle_controller_chain(&mut self, snapshot: Level2CastleState, level: u8) {
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
            let ground = self.terrain.ground_height(stage_position);
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
                source_byte_3e: stage_slot as u8,
                source_byte_3b: 0,
                cave_collision_cooldown: 0,
                source_dword_10: 0,
                source_word_2e: 0,
                construction_target_height: 0,
                source_subspell_index: 0,
                yaw: 0,
                roll: 0,
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
    fn retire_castle_defender_to_mana(&mut self, source_slot: u16) -> bool {
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
    pub(super) fn maintain_castle_defenders(&mut self, castle_slot: u16) {
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
                    self.retire_castle_defender_to_mana(related_slot);
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
                    source_byte_3e: snapshot.phase,
                    source_byte_3b: 0,
                    cave_collision_cooldown: 0,
                    source_dword_10: 0,
                    source_word_2e: 0,
                    construction_target_height: 0,
                    source_subspell_index: 0,
                    yaw: 0,
                    roll: 0,
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
                self.retire_castle_defender_to_mana(slot);
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
                        z: self.terrain.ground_height(child_position),
                        ..child_position
                    };
                    if let Some(slot) = self.allocate_slot() {
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
                            life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                            max_life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                            speed: LEVEL2_CASTLE_CHILD_DEFAULT_SPEED,
                            target_sphere_slot: 0,
                            source_byte_3e: slot as u8,
                            source_byte_3b: 0,
                            cave_collision_cooldown: 0,
                            source_dword_10: 0,
                            source_word_2e: 0,
                            construction_target_height: 0,
                            source_subspell_index: 0,
                            yaw: 512,
                            roll: 512,
                            // `sub_4C1E0` sets byte[2] & 2, making this
                            // class-5/model-15 row eligible for the
                            // `NewEvent_4A050` replaceable fallback.
                            control_flags: SOURCE_REPLACEABLE_FLAG,
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
    pub fn step_castle_action(&mut self, castle_slot: u16) -> bool {
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
                    source_byte_3e: child_slot as u8,
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
                        let ground = self.terrain.ground_height(stage_position);
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
                            source_byte_3e: stage_slot as u8,
                            source_byte_3b: 0,
                            cave_collision_cooldown: 0,
                            source_dword_10: 0,
                            source_word_2e: 0,
                            construction_target_height: 0,
                            source_subspell_index: 0,
                            yaw: 0,
                            roll: 0,
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
                castle.position.z = self.terrain.ground_height(castle.position);
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
                    source_byte_3e: child_slot as u8,
                    source_byte_3b: 0,
                    cave_collision_cooldown: snapshot.level.max(1),
                    source_dword_10: 0,
                    source_word_2e: 0,
                    construction_target_height: 0,
                    source_subspell_index: 0,
                    yaw: 0,
                    roll: 0,
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
                    .z = self.terrain.ground_height(snapshot.position);
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
                castle.position.z = self.terrain.ground_height(castle.position);
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
                        castle.position.z = self.terrain.ground_height(castle.position);
                        self.advance_castle_phase(castle_slot);
                        return true;
                    }
                }
                if source_phase & 1 != 0 {
                    self.advance_castle_phase(castle_slot);
                    return true;
                }
                if let Some(castle) = self.castle.as_mut() {
                    castle.position.z = self.terrain.ground_height(castle.position);
                }
                self.release_castle_excess_mana(castle_slot);
                self.maintain_castle_defenders(castle_slot);
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
                self.spheres[sphere_index].active = false;
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
                self.prepare_castle_stage_destruction(castle_slot);
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
                            &mut self.terrain.planes,
                            start_x,
                            start_y,
                            &footprint,
                            self.terrain.map_type,
                            &mut self.terrain.terrain_rng,
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
                    self.spawn_castle_controller_chain(downgraded, next_level);
                }
                // `sub_5FCA0` performs a second overflow release and then
                // runs `sub_5FF50` after `sub_605E0` has rebuilt the lower
                // stage. Allocation failure here must not roll back the
                // already-committed stage transition.
                self.release_castle_excess_mana(castle_slot);
                self.maintain_castle_defenders(castle_slot);
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
    pub fn step_carrier_to_castle(&mut self, carrier_slot: u16, position: Position) -> bool {
        let Some(castle) = self.castle.filter(|castle| castle.active) else {
            return false;
        };
        let raw_position = position;
        let ground = self.terrain.ground_height(raw_position);
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
mod tests {
    use super::super::tests::level_two_fixture_path;
    use super::*;

    fn level_two_possession_lifecycle_session() -> Level2ResetSession {
        let package = level_two_fixture_path();
        let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
            .expect("qualified Level 2 gameplay component")
            .level2_reset_session()
            .expect("Level 2 possession lifecycle session")
    }

    #[test]
    fn level_two_possession_contact_retry_preserves_shifted_life_and_prologue_reclaim() {
        let mut session = level_two_possession_lifecycle_session();
        let source_id_entity_index = session.state.entity_index;
        session.activate_level2_authored_switch_group(0, 23, 1);
        let target = session
            .economy()
            .spheres()
            .iter()
            .copied()
            .find(|sphere| {
                sphere.active
                    && !sphere.pending_delete
                    && u16::try_from(sphere.id).ok() != Some(source_id_entity_index)
                    && sphere.player_entity_index != source_id_entity_index
            })
            .expect("fixture has an eligible model-39 possession target");
        session
            .economy_mut()
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == target.source_entity_index)
            .expect("eligible contact target remains mutable")
            .owner_entity_index = source_id_entity_index;
        // `sub_108B0` starts at the rounded map cell while map linking uses
        // the source position's high bytes.  Put the test flyer in the
        // target's class-0 cell rather than relying on the now-forbidden
        // class-1 backtrack for a low-byte edge position.
        let projectile_position = Position {
            x: target.position.x & 0xff00,
            y: target.position.y & 0xff00,
            z: target.position.z,
        };
        let projectile_slot = session
            .economy_mut()
            .spawn_possession_projectile(
                projectile_position,
                source_id_entity_index,
                116,
                0,
                0,
                -POSSESSION_PROJECTILE_SPEED,
                projectile_position,
            )
            .expect("source arena admits possession flyer");
        {
            let economy = session.economy_mut();
            let projectile = economy
                .possession_projectiles
                .iter_mut()
                .find(|projectile| projectile.source_entity_index == projectile_slot)
                .expect("spawned possession flyer");
            projectile.position = projectile_position;
            projectile.actual_speed = 0;
            economy
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: projectile_slot,
                    active: true,
                    x: projectile_position.x,
                    y: projectile_position.y,
                })
                .expect("flyer source slot links to the live map");
            let projectile = *economy
                .possession_projectiles
                .iter()
                .find(|projectile| projectile.source_entity_index == projectile_slot)
                .expect("source predicate probe flyer");
            assert_eq!(
                level2_first_possession_target_sphere_slot(
                    &economy.map_chains,
                    &economy.spheres,
                    projectile,
                ),
                Some(target.source_entity_index),
                "an already player-owned target remains eligible when its raw id differs"
            );
            economy
                .spheres
                .iter_mut()
                .find(|sphere| sphere.source_entity_index == target.source_entity_index)
                .expect("source predicate target row")
                .player_entity_index = source_id_entity_index;
            assert!(
                level2_first_possession_target_sphere_slot(
                    &economy.map_chains,
                    &economy.spheres,
                    projectile,
                )
                .is_none(),
                "the raw playerEntityIndex relation, not owner equality, excludes the target"
            );
            economy
                .spheres
                .iter_mut()
                .find(|sphere| sphere.source_entity_index == target.source_entity_index)
                .expect("source predicate target row")
                .player_entity_index = 0;
        }
        let free_after_flyer = session.economy().free_slots.clone();
        let life_before_contact = session
            .economy()
            .possession_projectiles()
            .iter()
            .find(|projectile| projectile.source_entity_index == projectile_slot)
            .expect("live flyer before failed contact")
            .life;
        session.economy_mut().free_slots.clear();

        assert!(
            session
                .economy_mut()
                .step_possession_projectile_slot(projectile_slot),
            "failed model-12 allocation keeps the collision flyer live"
        );
        let retry = session
            .economy()
            .possession_projectiles()
            .iter()
            .find(|projectile| projectile.source_entity_index == projectile_slot)
            .copied()
            .expect("failed contact retains flyer for a later source callback");
        assert!(retry.active);
        assert_eq!(
            retry.life, life_before_contact,
            "contact does not decrement life"
        );
        assert_eq!(
            retry.position.z,
            target.position.z.wrapping_add(POSSESSION_TARGET_Z_SHIFT),
            "source contact copies the temporary model-39 +28 Z without mutating the target"
        );
        assert_eq!(
            session
                .economy()
                .spheres()
                .iter()
                .find(|sphere| sphere.source_entity_index == target.source_entity_index)
                .expect("contact target remains live")
                .position,
            target.position,
            "sub_655A0 restores the model-39 raw position after contact"
        );
        assert!(session.economy().possession_effects().is_empty());

        session.economy_mut().free_slots = free_after_flyer;
        assert!(
            session
                .economy_mut()
                .step_possession_projectile_slot(projectile_slot),
            "the next ordinary callback is not forced to re-hit a target after its snapped map-cell position changes"
        );
        let ordinary_next_callback = session
            .economy()
            .possession_projectiles()
            .iter()
            .find(|projectile| projectile.source_entity_index == projectile_slot)
            .copied()
            .expect("ordinary callback keeps the no-longer-contacting flyer live");
        assert!(ordinary_next_callback.active);
        assert_eq!(
            ordinary_next_callback.life,
            life_before_contact.wrapping_sub(1),
            "after leaving target contact, the ordinary no-target branch resumes its signed life decrement"
        );
        assert!(
            session.economy().possession_effects().is_empty(),
            "a fresh callback does not synthesize a second forced target impact"
        );

        let mut retired_from_expiry = false;
        for _ in 0..=usize::try_from(life_before_contact)
            .expect("possession fixture lifetime is non-negative")
        {
            if !session
                .economy_mut()
                .step_possession_projectile_slot(projectile_slot)
            {
                retired_from_expiry = true;
                break;
            }
        }
        assert!(
            retired_from_expiry,
            "the ordinary no-target lifetime eventually allocates its one model-12 effect"
        );
        let retired = session
            .economy()
            .possession_projectiles()
            .iter()
            .find(|projectile| projectile.source_entity_index == projectile_slot)
            .copied()
            .expect("disabled flyer remains represented until the next prologue");
        assert!(!retired.active);
        assert!(
            session.economy().map_chains().is_linked(projectile_slot),
            "DisableEntityDrawing leaves the disabled source row map-linked in this pass"
        );
        assert!(
            !session.economy().free_slots.contains(&projectile_slot),
            "disabled source slot remains unavailable before the next prologue"
        );
        assert_eq!(
            session
                .economy()
                .possession_effects()
                .iter()
                .find(|effect| effect.projectile_entity_index == projectile_slot)
                .expect("eventual no-target expiry creates one model-12 effect")
                .position
                .z,
            target.position.z.wrapping_add(POSSESSION_TARGET_Z_SHIFT),
            "model-12 is created at the shifted contact position"
        );

        session
            .economy_mut()
            .reclaim_possession_rows_at_source_boundary();
        assert!(session.economy().possession_projectiles().is_empty());
        assert!(
            !session.economy().map_chains().is_linked(projectile_slot),
            "the following source-pass prologue alone unlinks the disabled flyer"
        );
        assert!(
            session.economy().free_slots.contains(&projectile_slot),
            "the following source-pass prologue alone releases the slot"
        );
    }

    #[test]
    fn level_two_possession_expiry_retries_model12_allocation() {
        let mut session = level_two_possession_lifecycle_session();
        let source_id_entity_index = session.state.entity_index;
        let no_target_position = Position {
            x: 32_000,
            y: 32_000,
            z: 30_000,
        };
        let projectile_slot = session
            .economy_mut()
            .spawn_possession_projectile(
                no_target_position,
                source_id_entity_index,
                116,
                0,
                0,
                -POSSESSION_PROJECTILE_SPEED,
                no_target_position,
            )
            .expect("source arena admits possession flyer");
        {
            let economy = session.economy_mut();
            let projectile = economy
                .possession_projectiles
                .iter_mut()
                .find(|projectile| projectile.source_entity_index == projectile_slot)
                .expect("spawned possession flyer");
            projectile.position = no_target_position;
            projectile.actual_speed = 0;
            projectile.life = 0;
            assert!(
                level2_first_possession_target_sphere_slot(
                    &economy.map_chains,
                    &economy.spheres,
                    *projectile,
                )
                .is_none(),
                "expiry fixture is outside every eligible model-39 collision volume"
            );
            economy
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: projectile_slot,
                    active: true,
                    x: no_target_position.x,
                    y: no_target_position.y,
                })
                .expect("expiry flyer source slot links to the live map");
        }
        let free_after_flyer = session.economy().free_slots.clone();
        session.economy_mut().free_slots.clear();

        assert!(
            session
                .economy_mut()
                .step_possession_projectile_slot(projectile_slot),
            "expired flyer remains live when model-12 allocation fails"
        );
        let retry = session
            .economy()
            .possession_projectiles()
            .iter()
            .find(|projectile| projectile.source_entity_index == projectile_slot)
            .copied()
            .expect("failed expiry retains flyer");
        assert_eq!(
            retry.life, -1,
            "no-target expiry decrements before allocation"
        );
        assert!(session.economy().possession_effects().is_empty());

        session.economy_mut().free_slots = free_after_flyer;
        assert!(
            !session
                .economy_mut()
                .step_possession_projectile_slot(projectile_slot),
            "the expiry branch retries model-12 allocation on its next callback"
        );
        assert!(
            session
                .economy()
                .possession_effects()
                .iter()
                .any(|effect| effect.projectile_entity_index == projectile_slot),
            "no-target expiry produces the same typed model-12 effect"
        );
    }

    #[test]
    fn level_two_possession_strict_terrain_and_cave_clamps_still_decrement_life() {
        fn assert_no_target_clamp_steps_lifetime(
            session: &mut Level2ResetSession,
            position: Position,
            expected_z: i16,
            boundary_case: &str,
        ) {
            let source_id_entity_index = session.state.entity_index;
            let projectile_slot = session
                .economy_mut()
                .spawn_possession_projectile(
                    position,
                    source_id_entity_index,
                    116,
                    0,
                    0,
                    -POSSESSION_PROJECTILE_SPEED,
                    position,
                )
                .expect("source arena admits possession flyer");
            {
                let economy = session.economy_mut();
                let projectile = economy
                    .possession_projectiles
                    .iter_mut()
                    .find(|projectile| projectile.source_entity_index == projectile_slot)
                    .expect("spawned possession flyer");
                projectile.position = position;
                projectile.actual_speed = 0;
                projectile.life = 4;
                assert!(
                    level2_first_possession_target_sphere_slot(
                        &economy.map_chains,
                        &economy.spheres,
                        *projectile,
                    )
                    .is_none(),
                    "{boundary_case} fixture is outside every eligible model-39 collision volume"
                );
                economy
                    .map_chains
                    .sync_entity(RenderableMapSeed {
                        entity_index: projectile_slot,
                        active: true,
                        x: position.x,
                        y: position.y,
                    })
                    .expect("contact flyer source slot links to the live map");
            }

            assert!(
                session
                    .economy_mut()
                    .step_possession_projectile_slot(projectile_slot),
                "an initial {boundary_case} clamp keeps the no-target flyer live"
            );
            let stepped = session
                .economy()
                .possession_projectiles()
                .iter()
                .find(|projectile| projectile.source_entity_index == projectile_slot)
                .copied()
                .expect("clamped flyer remains live after its ordinary callback");
            assert!(stepped.active);
            assert_eq!(
                stepped.life, 3,
                "{boundary_case} clamps still take the signed no-target life decrement"
            );
            assert_eq!(
                stepped.position.z, expected_z,
                "{boundary_case} applies only its strict source Z clamp"
            );
            assert!(
                !session
                    .economy()
                    .possession_effects()
                    .iter()
                    .any(|effect| effect.projectile_entity_index == projectile_slot),
                "{boundary_case} does not manufacture model 12 before target, post-scan conflict, or expiry"
            );
        }

        let contact_position = Position {
            x: 32_128,
            y: 32_000,
            z: 1_599,
        };
        let mut ground_session = level_two_possession_lifecycle_session();
        ground_session
            .economy_mut()
            .terrain
            .planes
            .ground_heights
            .fill(50);
        assert_no_target_clamp_steps_lifetime(
            &mut ground_session,
            contact_position,
            1_600,
            "below ground",
        );
        assert_no_target_clamp_steps_lifetime(
            &mut ground_session,
            Position {
                z: 1_600,
                ..contact_position
            },
            1_600,
            "equal ground",
        );

        let mut cave_session = level_two_possession_lifecycle_session();
        {
            let terrain = &mut cave_session.economy_mut().terrain;
            terrain.map_type = MapType::Cave;
            terrain.planes.ground_heights.fill(0);
            terrain.planes.secondary_heights.fill(255);
        }
        let cave_limit = 8_160_i16.wrapping_sub(POSSESSION_PROJECTILE_COLLISION_HALF_HEIGHT);
        assert_no_target_clamp_steps_lifetime(
            &mut cave_session,
            Position {
                z: 8_000,
                ..contact_position
            },
            cave_limit,
            "above cave limit",
        );
        assert_no_target_clamp_steps_lifetime(
            &mut cave_session,
            Position {
                z: cave_limit,
                ..contact_position
            },
            cave_limit,
            "equal cave limit",
        );
    }

    #[test]
    fn level_two_possession_uses_only_class0_search_cells_before_first_target() {
        let mut session = level_two_possession_lifecycle_session();
        let source_id_entity_index = session.state.entity_index;
        let projectile_position = Position {
            x: 32_000,
            y: 32_000,
            z: 30_000,
        };
        let projectile_slot = session
            .economy_mut()
            .spawn_possession_projectile(
                projectile_position,
                source_id_entity_index,
                116,
                0,
                0,
                -POSSESSION_PROJECTILE_SPEED,
                projectile_position,
            )
            .expect("source arena admits possession flyer");
        {
            let economy = session.economy_mut();
            let projectile = economy
                .possession_projectiles
                .iter_mut()
                .find(|projectile| projectile.source_entity_index == projectile_slot)
                .expect("spawned possession flyer");
            projectile.position = projectile_position;
            projectile.actual_speed = 0;
            economy
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: projectile_slot,
                    active: true,
                    x: projectile_position.x,
                    y: projectile_position.y,
                })
                .expect("projectile source row links to the live map");
        }
        let class1_only_position = Position {
            x: projectile_position.x.wrapping_sub(1),
            ..projectile_position
        };
        let class1_only = session
            .economy_mut()
            .allocate_authored_model39(0, 23, class1_only_position)
            .expect("source arena admits class-1-only candidate");
        {
            let candidate = session
                .economy_mut()
                .spheres
                .iter_mut()
                .find(|sphere| {
                    sphere.source_entity_index == class1_only.source_entity_index
                        && sphere.position == class1_only_position
                })
                .expect("class-1-only candidate remains mutable");
            candidate.id = 0xfeed;
            candidate.player_entity_index = source_id_entity_index.wrapping_add(1);
        }
        let projectile = session
            .economy()
            .possession_projectiles()
            .iter()
            .copied()
            .find(|projectile| projectile.source_entity_index == projectile_slot)
            .expect("live possession flyer");
        let economy = session.economy();
        assert!(
            source_map_search_slots(
                economy.map_chains(),
                projectile_position,
                &LEVEL2_MANA_SPHERE_SEARCH_OFFSETS,
            )
            .contains(&class1_only.source_entity_index),
            "the discriminating candidate is visible to the wider class-1 sphere walk"
        );
        assert!(
            !source_map_search_slots(
                economy.map_chains(),
                projectile_position,
                &LEVEL2_POSSESSION_PROJECTILE_SEARCH_OFFSETS,
            )
            .contains(&class1_only.source_entity_index),
            "the discriminating candidate is outside the class-0 possession envelope"
        );
        assert_eq!(
            level2_first_possession_target_sphere_slot(
                economy.map_chains(),
                economy.spheres(),
                projectile
            ),
            None,
            "a class-1-only model-39 must not become a Possession target"
        );

        let class0_first = session
            .economy_mut()
            .allocate_authored_model39(0, 23, projectile_position)
            .expect("source arena admits first class-0 candidate");
        {
            let candidate = session
                .economy_mut()
                .spheres
                .iter_mut()
                .find(|sphere| {
                    sphere.source_entity_index == class0_first.source_entity_index
                        && sphere.position == projectile_position
                })
                .expect("class-0 candidate remains mutable");
            candidate.id = 0xbeef;
            candidate.player_entity_index = source_id_entity_index.wrapping_add(1);
        }
        let economy = session.economy();
        assert!(
            source_map_search_slots(
                economy.map_chains(),
                projectile_position,
                &LEVEL2_POSSESSION_PROJECTILE_SEARCH_OFFSETS,
            )
            .contains(&class0_first.source_entity_index),
            "first class-0 candidate must be in the possession envelope"
        );
        assert_eq!(
            level2_first_possession_target_sphere_slot(
                economy.map_chains(),
                economy.spheres(),
                projectile
            ),
            Some(class0_first.source_entity_index),
            "the first eligible class-0 model-39 is the sole target candidate"
        );
    }

    #[test]
    fn level_two_possession_guidance_acquires_once_turns_then_keeps_stale_slot() {
        let mut session = level_two_possession_lifecycle_session();
        let baseline_hash = session.economy().reset_hash();
        let source_id_entity_index = session.state.entity_index;
        let projectile_position = Position {
            x: 32_000,
            y: 32_000,
            z: 24_000,
        };
        let first_position = Position {
            x: projectile_position.x.wrapping_sub(128),
            y: projectile_position.y.wrapping_sub(2_000),
            z: projectile_position.z,
        };
        let second_position = Position {
            x: projectile_position.x.wrapping_add(128),
            ..first_position
        };
        let first = session
            .economy_mut()
            .allocate_authored_model39(0, 65, first_position)
            .expect("first equal-score model-39 guidance target");
        let second = session
            .economy_mut()
            .allocate_authored_model39(0, 65, second_position)
            .expect("second equal-score model-39 guidance target");
        assert!(first.source_entity_index < second.source_entity_index);
        for slot in [first.source_entity_index, second.source_entity_index] {
            let sphere = session
                .economy_mut()
                .spheres
                .iter_mut()
                .find(|sphere| sphere.source_entity_index == slot)
                .expect("guidance candidate remains represented");
            sphere.player_entity_index = source_id_entity_index.wrapping_add(1);
            sphere.velocity_x = 0;
            sphere.velocity_y = 0;
            sphere.vertical_velocity = 0;
        }
        session
            .economy_mut()
            .rebuild_possession_guidance_source_slots();
        let projectile_slot = session
            .economy_mut()
            .spawn_possession_projectile(
                projectile_position,
                source_id_entity_index,
                116,
                0,
                0,
                0,
                projectile_position,
            )
            .expect("guided Possession flyer allocation");
        {
            let projectile = session
                .economy_mut()
                .possession_projectiles
                .iter_mut()
                .find(|projectile| projectile.source_entity_index == projectile_slot)
                .expect("guided flyer remains represented");
            assert_eq!(projectile.position, projectile_position);
        }
        let before_first = session
            .economy()
            .possession_projectiles()
            .iter()
            .find(|projectile| projectile.source_entity_index == projectile_slot)
            .copied()
            .expect("guided flyer before first callback");
        let first_target_top = level2_possession_guidance_target_top(
            session
                .economy()
                .spheres()
                .iter()
                .find(|sphere| sphere.source_entity_index == first.source_entity_index)
                .expect("lower-slot guidance target"),
        );
        let expected_first_angles =
            projectile_guidance_angles(before_first.position, first_target_top);
        let mut expected_first_position = before_first.position;
        crate::move_entity(
            &mut expected_first_position,
            expected_first_angles.0,
            expected_first_angles.1,
            before_first.actual_speed,
        );

        assert!(
            session
                .economy_mut()
                .step_possession_projectile_slot(projectile_slot),
            "the first guided callback remains in flight"
        );
        let after_first = session
            .economy()
            .possession_projectiles()
            .iter()
            .find(|projectile| projectile.source_entity_index == projectile_slot)
            .copied()
            .expect("guided flyer after first callback");
        assert!(after_first.acquisition_attempted);
        assert_eq!(
            after_first.target_source_entity_index, first.source_entity_index,
            "strict-lower equal-score selection retains the lower source slot"
        );
        assert_eq!((after_first.roll, after_first.fov), expected_first_angles);
        assert_eq!((after_first.yaw, after_first.pitch), expected_first_angles);
        assert_eq!(
            after_first.position, expected_first_position,
            "first acquisition copies desired angles before the first move"
        );

        let moved_target_position = Position {
            x: after_first.position.x.wrapping_add(2_000),
            y: after_first.position.y.wrapping_sub(400),
            z: after_first.position.z,
        };
        session
            .economy_mut()
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == first.source_entity_index)
            .expect("selected guidance target before second callback")
            .position = moved_target_position;
        let moved_target_top = level2_possession_guidance_target_top(
            session
                .economy()
                .spheres()
                .iter()
                .find(|sphere| sphere.source_entity_index == first.source_entity_index)
                .expect("moved guidance target"),
        );
        let expected_second_angles =
            projectile_guidance_angles(after_first.position, moved_target_top);
        let expected_second_yaw = after_first.yaw.wrapping_add(turn_toward(
            after_first.yaw,
            expected_second_angles.0,
            PROJECTILE_ACQUISITION_ANGLE,
        )) & 0x07ff;
        let expected_second_pitch = after_first.pitch.wrapping_add(turn_toward(
            after_first.pitch,
            expected_second_angles.1,
            PROJECTILE_ACQUISITION_ANGLE,
        )) & 0x07ff;
        assert_ne!(
            expected_second_yaw, expected_second_angles.0,
            "the moved target requires a capped rather than snapping yaw turn"
        );
        assert!(
            session
                .economy_mut()
                .step_possession_projectile_slot(projectile_slot),
            "the second guided callback remains in flight"
        );
        let after_second = session
            .economy()
            .possession_projectiles()
            .iter()
            .find(|projectile| projectile.source_entity_index == projectile_slot)
            .copied()
            .expect("guided flyer after second callback");
        assert_eq!(
            (after_second.roll, after_second.fov),
            expected_second_angles
        );
        assert_eq!(after_second.yaw, expected_second_yaw);
        assert_eq!(after_second.pitch, expected_second_pitch);

        session
            .economy_mut()
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == first.source_entity_index)
            .expect("selected target before reclaim")
            .active = false;
        session.economy_mut().reclaim_inactive_slots();
        let stale_target_top = session
            .economy()
            .possession_guidance_stale_records
            .get(&first.source_entity_index)
            .copied()
            .expect("source fixed-slot record survives model-39 reclamation");
        let expected_stale_angles =
            projectile_guidance_angles(after_second.position, stale_target_top);
        let _ = session
            .economy_mut()
            .step_possession_projectile_slot(projectile_slot);
        let after_reclaim = session
            .economy()
            .possession_projectiles()
            .iter()
            .find(|projectile| projectile.source_entity_index == projectile_slot)
            .copied()
            .expect("guided flyer remains represented after target reclaim");
        assert!(after_reclaim.acquisition_attempted);
        assert_eq!(
            after_reclaim.target_source_entity_index, first.source_entity_index,
            "a stale source relation never reacquires the remaining candidate"
        );
        assert_ne!(
            after_reclaim.target_source_entity_index, second.source_entity_index,
            "candidate B cannot replace the one-shot source relation"
        );
        assert_eq!(
            (after_reclaim.roll, after_reclaim.fov),
            expected_stale_angles,
            "a reclaimed source slot continues steering through its stale fixed record"
        );

        let replacement_position = Position {
            x: after_reclaim.position.x.wrapping_add(600),
            y: after_reclaim.position.y.wrapping_sub(1_500),
            z: after_reclaim.position.z.wrapping_add(37),
        };
        let replacement_slot = session
            .economy_mut()
            .spawn_possession_projectile(
                replacement_position,
                source_id_entity_index,
                116,
                0,
                0,
                0,
                Position {
                    z: replacement_position.z.wrapping_add(999),
                    ..replacement_position
                },
            )
            .expect("released guidance slot can be overwritten by an ordinary model-1 NewEvent");
        assert_eq!(replacement_slot, first.source_entity_index);
        let replacement_top = replacement_position;
        let expected_replacement_angles =
            projectile_guidance_angles(after_reclaim.position, replacement_top);
        let _ = session
            .economy_mut()
            .step_possession_projectile_slot(projectile_slot);
        let after_reuse = session
            .economy()
            .possession_projectiles()
            .iter()
            .find(|projectile| projectile.source_entity_index == projectile_slot)
            .copied()
            .expect("guided flyer follows the reused source record");
        assert_eq!(
            (after_reuse.roll, after_reuse.fov),
            expected_replacement_angles,
            "slot reuse changes the existing relation's desired angles without reacquisition"
        );
        assert!(after_reuse.acquisition_attempted);
        assert_eq!(after_reuse.target_source_entity_index, replacement_slot);
        assert_ne!(session.economy().reset_hash(), baseline_hash);
        session.reset();
        assert_eq!(session.economy().reset_hash(), baseline_hash);
    }

    #[test]
    fn level_two_possession_holder_charges_once_then_clears_positive_cooldown_regen() {
        let mut session = level_two_possession_lifecycle_session();
        let holder_index = 116;
        let (activation_value, mana_cost) = {
            let holder = session
                .shared_spells_mut()
                .dynamic_pickups
                .iter_mut()
                .find(|holder| holder.index == holder_index)
                .expect("ordinary Level 2 Possession holder");
            let activation_value = i16::try_from(holder.activation_value)
                .expect("ordinary Possession activation value fits signed source field");
            holder.active = true;
            holder.active_value = activation_value;
            holder.life = 0;
            (activation_value, holder.mana_cost)
        };
        assert!(mana_cost > 0, "fixture retains a positive source mana cost");
        {
            let shared = session.shared_spells_mut();
            shared.player_entity.mana_regen = 37;
            shared.player_entity.record[140..144].copy_from_slice(&37_i32.to_le_bytes());
            for entity in &mut shared.player_entities {
                if entity.index == shared.player_entity.index {
                    entity.mana_regen = 37;
                    entity.record[140..144].copy_from_slice(&37_i32.to_le_bytes());
                }
            }
        }

        session.step_level2_possession_holder_source_slot(holder_index);
        assert_eq!(
            session.shared_spells().player_entity.mana_regen,
            -mana_cost,
            "the activation tick applies the one source mana charge"
        );
        assert_eq!(
            session
                .shared_spells()
                .dynamic_pickups
                .iter()
                .find(|holder| holder.index == holder_index)
                .expect("active possession holder after source callback")
                .active_value,
            activation_value - 1,
            "spawn attempt still precedes the source countdown decrement"
        );
        assert_eq!(
            session
                .economy()
                .possession_projectiles()
                .iter()
                .filter(|projectile| projectile.holder_entity_index == holder_index)
                .count(),
            1,
            "the activation tick creates exactly one possession flyer"
        );

        {
            let shared = session.shared_spells_mut();
            shared.player_entity.mana_regen = 37;
            shared.player_entity.record[140..144].copy_from_slice(&37_i32.to_le_bytes());
            for entity in &mut shared.player_entities {
                if entity.index == shared.player_entity.index {
                    entity.mana_regen = 37;
                    entity.record[140..144].copy_from_slice(&37_i32.to_le_bytes());
                }
            }
        }
        session.step_level2_possession_holder_source_slot(holder_index);
        assert_eq!(
            session.shared_spells().player_entity.mana_regen,
            0,
            "a positive regeneration value is cleared on a later active cooldown tick"
        );
        assert_eq!(
            session
                .economy()
                .possession_projectiles()
                .iter()
                .filter(|projectile| projectile.holder_entity_index == holder_index)
                .count(),
            1,
            "cooldown ticks do not create a second possession flyer"
        );
    }

    #[test]
    fn level_two_possession_effect_writes_every_overlapping_live_sphere_mailbox() {
        let mut session = level_two_possession_lifecycle_session();
        let source_id_entity_index = session.state.entity_index;
        let probe_position = Position {
            x: 32_000,
            y: 32_000,
            z: 0,
        };
        let collision_position = Position {
            z: session.economy().terrain.ground_height(probe_position),
            ..probe_position
        };
        let first = session
            .economy_mut()
            .allocate_authored_model39(0, 23, collision_position)
            .expect("first map-linked model-39 source for Possession effect fan-out");
        let second = session
            .economy_mut()
            .allocate_authored_model39(0, 23, collision_position)
            .expect("second live model-39 source in the same effect volume");
        for (sphere_slot, fixture_id) in [
            (first.source_entity_index, 0xfeed_0001),
            (second.source_entity_index, 0xfeed_0002),
        ] {
            let sphere = session
                .economy_mut()
                .spheres
                .iter_mut()
                .find(|sphere| sphere.source_entity_index == sphere_slot)
                .expect("map-linked fan-out fixture remains mutable");
            sphere.id = fixture_id;
            sphere.player_entity_index = source_id_entity_index.wrapping_add(1);
        }
        let projectile_slot = session
            .economy_mut()
            .spawn_possession_projectile(
                collision_position,
                source_id_entity_index,
                116,
                0,
                0,
                -POSSESSION_PROJECTILE_SPEED,
                collision_position,
            )
            .expect("source arena admits possession flyer");
        {
            let economy = session.economy_mut();
            let projectile = economy
                .possession_projectiles
                .iter_mut()
                .find(|projectile| projectile.source_entity_index == projectile_slot)
                .expect("spawned possession flyer");
            projectile.position = collision_position;
            projectile.actual_speed = 0;
            economy
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: projectile_slot,
                    active: true,
                    x: collision_position.x,
                    y: collision_position.y,
                })
                .expect("flyer source slot links to the live map");
        }
        assert!(
            !session
                .economy_mut()
                .step_possession_projectile_slot(projectile_slot),
            "a real model-39 target creates model-12 through the flyer callback"
        );
        let effect_slot = session
            .economy()
            .possession_effects()
            .iter()
            .find(|effect| effect.projectile_entity_index == projectile_slot)
            .expect("target collision effect")
            .source_entity_index;
        assert!(
            session
                .economy_mut()
                .step_possession_effect_slot(effect_slot),
            "first model-12 callback remains live after mailbox writes"
        );
        for sphere_slot in [first.source_entity_index, second.source_entity_index] {
            let sphere = session
                .economy()
                .spheres()
                .iter()
                .find(|sphere| sphere.source_entity_index == sphere_slot)
                .expect("overlapping map-linked model-39 source");
            assert_eq!(
                sphere.incoming_owner_entity_index, source_id_entity_index,
                "model-12 writes its source id to every overlapping sphere, not only its collision target"
            );
            assert_eq!(sphere.incoming_possession_payload, 0);
        }
    }

    #[test]
    fn level_two_possession_new_rows_follow_the_remaining_ascending_source_cursor() {
        fn run_case(target_after_projectile: bool) -> (u16, u16, u16, u16) {
            let mut session = level_two_possession_lifecycle_session();
            // Let the owned Level 2 bootstrap materialize its pre-existing
            // source rows first. The cursor cases below then isolate a real
            // later `NewEvent` insertion without competing with unrelated
            // first-pass authored constructors.
            session.step_level2_source_pass(0);
            let source_id_entity_index = session.state.entity_index;
            let mut static_route_slots = BTreeSet::from([source_id_entity_index, 38_u16, 75_u16]);
            static_route_slots.extend(
                session
                    .shared_spells
                    .dynamic_pickups
                    .iter()
                    .filter(|holder| {
                        holder.active
                            && holder.class == 15
                            && holder.model == POSSESSION_SPELL_FAMILY
                            && holder.action == POSSESSION_SPELL_FAMILY.saturating_mul(3)
                    })
                    .map(|holder| holder.index),
            );
            static_route_slots.extend(
                session
                    .entities
                    .iter()
                    .filter(|entity| {
                        entity.active
                            && entity.class == 3
                            && entity.model == 1
                            && entity.action == 1
                    })
                    .map(|entity| entity.index),
            );
            static_route_slots.extend(session.bad_stones.iter().map(|stone| stone.entity.index));
            static_route_slots.extend(
                session
                    .ordinary
                    .houses()
                    .iter()
                    .map(|house| house.entity_index),
            );
            static_route_slots.extend(
                session
                    .ordinary
                    .villagers()
                    .iter()
                    .map(|villager| villager.entity_index),
            );
            static_route_slots.extend(
                session
                    .ordinary
                    .dolmens()
                    .iter()
                    .map(|dolmen| dolmen.entity.index),
            );
            static_route_slots.extend(
                session
                    .economy()
                    .castle()
                    .filter(|castle| castle.active)
                    .map(|castle| castle.source_entity_index),
            );
            static_route_slots.extend(LEVEL_TWO_GOAT_INDICES);
            let mut slots = session
                .economy()
                .free_slots
                .iter()
                .copied()
                .filter(|slot| !static_route_slots.contains(slot))
                .collect::<Vec<_>>();
            slots.sort_unstable();
            assert!(
                slots.len() >= 3,
                "fixture provides three non-static source slots for cursor ordering"
            );
            let (projectile_slot, effect_slot, target_slot) = if target_after_projectile {
                (slots[0], slots[1], slots[2])
            } else {
                (slots[1], slots[2], slots[0])
            };
            let mut position = Position {
                x: 30_000,
                y: 30_000,
                z: 0,
            };
            position.z = session.terrain.ground_height(position);
            {
                let economy = session.economy_mut();
                economy.spheres.push(ManaSphere {
                    id: u32::from(target_slot),
                    source_entity_index: target_slot,
                    entity_rng: 0,
                    position,
                    velocity_x: 0,
                    velocity_y: 0,
                    vertical_velocity: 0,
                    yaw: 0,
                    mana: 512,
                    owner_entity_index: 0,
                    player_entity_index: 0,
                    incoming_owner_entity_index: 0,
                    incoming_possession_payload: 0,
                    attraction_source_entity_index: 0,
                    attraction_step: 0,
                    attraction_distance_high: 0,
                    assigned_carrier_id: 0,
                    assigned_defender_slot: 0,
                    control_flags: 0,
                    phase: 0,
                    active: true,
                    pending_delete: false,
                });
                economy
                    .possession_projectiles
                    .push(Level2PossessionProjectile {
                        source_entity_index: projectile_slot,
                        entity_rng: 0,
                        source_id_entity_index,
                        holder_entity_index: 116,
                        source_class: POSSESSION_PROJECTILE_CLASS,
                        source_model: POSSESSION_PROJECTILE_MODEL,
                        action: POSSESSION_PROJECTILE_ACTION,
                        output_class: POSSESSION_IMPACT_CLASS,
                        output_model: POSSESSION_IMPACT_MODEL,
                        target_source_entity_index: 0,
                        acquisition_attempted: false,
                        position,
                        axis_position: position,
                        yaw: 0,
                        pitch: 0,
                        roll: 0,
                        fov: 0,
                        actual_speed: 0,
                        minimum_speed: POSSESSION_PROJECTILE_SPEED,
                        life: POSSESSION_PROJECTILE_LIFETIME_TICKS,
                        max_life: POSSESSION_PROJECTILE_LIFETIME_TICKS,
                        phase: 0,
                        active: true,
                    });
                economy
                    .map_chains
                    .sync_entity(RenderableMapSeed {
                        entity_index: target_slot,
                        active: true,
                        x: position.x,
                        y: position.y,
                    })
                    .expect("synthetic model-39 is map-linked in source order");
                economy
                    .map_chains
                    .sync_entity(RenderableMapSeed {
                        entity_index: projectile_slot,
                        active: true,
                        x: position.x,
                        y: position.y,
                    })
                    .expect("synthetic possession flyer is map-linked in source order");
                // `allocate_slot` pops the last element. Force the effect
                // into the source slot between or after the two existing rows
                // without changing the actual flyer/effect constructor.
                economy.free_slots = vec![effect_slot];
                let projectile = economy
                    .possession_projectiles
                    .iter()
                    .find(|projectile| projectile.source_entity_index == projectile_slot)
                    .copied()
                    .expect("synthetic possession flyer remains live before source pass");
                assert_eq!(
                    level2_first_possession_target_sphere_slot(
                        &economy.map_chains,
                        &economy.spheres,
                        projectile,
                    ),
                    Some(target_slot),
                    "the controlled source-map fixture has one eligible collision target"
                );
                assert!(
                    economy
                        .live_arena_cursor_rows()
                        .iter()
                        .any(|row| row.source_slot == projectile_slot),
                    "the controlled flyer is visible to the initial dynamic cursor"
                );
            }

            session.step_level2_source_pass(1);
            let sphere_after_first = session
                .economy()
                .spheres()
                .iter()
                .find(|sphere| sphere.source_entity_index == target_slot)
                .expect("target sphere remains in the source arena after first pass");
            let owner_after_first = sphere_after_first.owner_entity_index;
            let mailbox_after_first = sphere_after_first.incoming_owner_entity_index;
            assert!(
                session
                    .economy()
                    .possession_effects()
                    .iter()
                    .any(|effect| effect.source_entity_index == effect_slot),
                "real collision allocation consumes the forced effect slot"
            );
            session.step_level2_source_pass(2);
            let owner_after_second = session
                .economy()
                .spheres()
                .iter()
                .find(|sphere| sphere.source_entity_index == target_slot)
                .expect("target sphere remains in the source arena after second pass")
                .owner_entity_index;
            (
                source_id_entity_index,
                owner_after_first,
                mailbox_after_first,
                owner_after_second,
            )
        }

        let (source_id, higher_owner, higher_mailbox, higher_owner_next) = run_case(true);
        assert_eq!(
            higher_owner, source_id,
            "S>E consumes the model-12 mailbox in the same source pass"
        );
        assert_eq!(
            higher_mailbox, 0,
            "same-pass sphere action clears its mailbox"
        );
        assert_eq!(higher_owner_next, source_id);

        let (source_id, lower_owner, lower_mailbox, lower_owner_next) = run_case(false);
        assert_ne!(
            lower_owner, source_id,
            "S<E has already passed its model-39 action when the effect is born"
        );
        assert_eq!(
            lower_mailbox, source_id,
            "S<E receives the source-id mailbox but cannot consume it early"
        );
        assert_eq!(
            lower_owner_next, source_id,
            "the following ascending source pass consumes the deferred mailbox"
        );
    }

    #[test]
    fn level_two_economy_projection_tracks_source_order_failure_rng_and_reset() {
        let package = level_two_fixture_path();
        let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
            .expect("qualified Level 2 gameplay component");
        assert!(
            inspection.level2_castle_stage_footprints.contains_key(&1),
            "Castle lifecycle projection requires the validated stage-1 BUILD footprint"
        );
        let mut first = inspection
            .level2_reset_session()
            .expect("Level 2 reset session");
        let second = inspection
            .level2_reset_session()
            .expect("independent Level 2 reset session");
        let baseline_hash = first.economy().reset_hash();
        let baseline_free_slots = first.economy().free_slot_count();
        assert_eq!(baseline_free_slots, second.economy().free_slot_count());
        assert_eq!(baseline_hash, second.economy().reset_hash());
        let raw_map_entries = raw_source_map_chain_entries(
            &inspection.terrain_planes().entity_indices,
            |source_entity_index| {
                inspection
                    .entities()
                    .get(usize::from(source_entity_index))
                    .map(EntitySnapshot::old_map_entity)
            },
        )
        .expect("validated Level 2 raw map chains");
        let holder_rows = first.shared_spells().dynamic_pickups().to_vec();
        let holder_slots = holder_rows
            .iter()
            .map(|holder| holder.index)
            .collect::<Vec<_>>();
        let mut expected_linked = raw_map_entries
            .iter()
            .map(|entry| entry.entity_index)
            .collect::<Vec<_>>();
        expected_linked.extend(holder_slots.iter().copied());
        expected_linked.sort_unstable();
        assert_eq!(
            first.economy().map_chains().linked_entity_indices(),
            expected_linked
        );
        let mut expected_by_tile = BTreeMap::<u16, Vec<u16>>::new();
        for entry in &raw_map_entries {
            expected_by_tile
                .entry(entry.tile)
                .or_default()
                .push(entry.entity_index);
        }
        for holder in &holder_rows {
            let tile = RenderableMapSeed {
                entity_index: holder.index,
                active: holder.active,
                x: holder.position.x,
                y: holder.position.y,
            }
            .tile_index();
            expected_by_tile
                .entry(tile)
                .or_default()
                .insert(0, holder.index);
        }
        for (tile, expected_chain) in expected_by_tile {
            assert_eq!(
                first
                    .economy()
                    .map_chains()
                    .entity_chain((tile & 0xff) as u8, (tile >> 8) as u8),
                expected_chain
            );
        }

        let sphere_slot = *first.economy().free_slots.last().expect("free sphere slot");
        let seed = sphere_slot.wrapping_add(inspection.runtime().random_state as u16);
        let sphere_rng = level2_next_rng16(seed);
        let expected_yaw =
            i16::try_from((i32::from(sphere_rng % 0x71) + i32::from(0x120_i16) - 56) & 0x07ff)
                .expect("sphere yaw");
        let sphere_id = first
            .economy_mut()
            .spawn_mana_sphere_from_death(
                42,
                Position {
                    x: 384,
                    y: 62_080,
                    z: 1_024,
                },
                0x120,
                0x4321,
                321,
                121,
            )
            .expect("sphere allocation");
        assert_eq!(sphere_id, u32::from(sphere_slot));
        let sphere = &first.economy().spheres()[0];
        assert_eq!(sphere.source_entity_index, sphere_slot);
        assert_eq!(sphere.entity_rng, level2_next_rng16(sphere_rng));
        assert_eq!(sphere.yaw, expected_yaw);
        assert_eq!(sphere.mana, 321);
        assert_eq!(sphere.owner_entity_index, 121);
        assert_eq!(first.economy().source_owner(42), 0);
        assert_eq!(
            first.economy().events()[0],
            Level2EconomyEvent::ManaSphereSpawned {
                sphere_id,
                source_entity_index: 42,
                mana: 321,
            }
        );
        assert_eq!(first.economy().free_slot_count(), baseline_free_slots - 1);

        assert!(first.economy_mut().claim_mana_sphere(sphere_slot, 111));
        assert_eq!(
            first.economy().events()[1],
            Level2EconomyEvent::ManaSphereClaimed {
                sphere_id,
                owner_entity_index: 111,
            }
        );
        assert!(!first.economy_mut().claim_mana_sphere(sphere_slot, 111));

        let carrier_slot = first
            .economy_mut()
            .spawn_mana_carrier(
                Position {
                    x: 384,
                    y: 62_080,
                    z: 1_024,
                },
                111,
                sphere_id,
            )
            .expect("carrier allocation");
        let carrier = first.economy().carrier().expect("active carrier");
        assert_eq!(carrier.source_entity_index, carrier_slot);
        assert_eq!(carrier.speed, MANA_CARRIER_SPEED);
        assert_eq!(carrier.life, MANA_CARRIER_MAX_LIFE);
        assert_eq!(carrier.max_mana, MANA_CARRIER_MAX_MANA);
        assert_eq!(
            first.economy().events()[2],
            Level2EconomyEvent::ManaCarrierSpawned {
                source_entity_index: carrier_slot,
                owner_entity_index: 111,
            }
        );
        assert!(
            first
                .economy_mut()
                .absorb_sphere_into_carrier(sphere_slot, carrier_slot)
        );
        assert_eq!(first.economy().spheres()[0].active, false);
        assert_eq!(first.economy().spheres()[0].assigned_carrier_id, 0);
        assert_eq!(
            first.economy().carrier().expect("carrier").carried_mana,
            321
        );
        assert_eq!(
            first.economy().events()[3],
            Level2EconomyEvent::ManaSphereCollected {
                sphere_id,
                carrier_id: u32::from(carrier_slot),
                mana: 321,
            }
        );

        let castle_slot = first
            .economy_mut()
            .build_castle(
                Position {
                    x: 1_024,
                    y: 1_024,
                    z: 0,
                },
                111,
            )
            .expect("castle allocation");
        assert_eq!(
            first.economy().castle().expect("castle").life,
            CASTLE_STAGE_ONE_LIFE
        );
        assert_eq!(
            first.economy().events()[4],
            Level2EconomyEvent::CastleBuilt {
                source_entity_index: castle_slot,
                owner_entity_index: 111,
            }
        );
        assert!(first.economy_mut().return_carrier_to_castle(carrier_slot));
        assert_eq!(first.economy().castle().expect("castle").mana, 0);
        assert!(first.economy().carrier().expect("returning carrier").active);
        let castle_position = first.economy().castle().expect("castle").position;
        let below_ground_far = Position {
            x: castle_position.x.wrapping_add(4_096),
            y: castle_position.y,
            z: i16::MIN,
        };
        let carrier_z_before_failed_move = first.economy().carrier().expect("carrier").position.z;
        assert!(
            !first
                .economy_mut()
                .step_carrier_to_castle(carrier_slot, below_ground_far)
        );
        assert_eq!(
            first
                .economy()
                .carrier()
                .expect("unchanged carrier")
                .position
                .z,
            carrier_z_before_failed_move
        );
        assert!(
            first
                .economy_mut()
                .step_carrier_to_castle(carrier_slot, castle_position)
        );
        assert_eq!(first.economy().castle().expect("castle").mana, 321);
        assert_eq!(
            first
                .economy()
                .carrier()
                .expect("arrived carrier")
                .carried_mana,
            0
        );
        assert_eq!(
            first.economy().events()[5],
            Level2EconomyEvent::ManaCarrierArrived {
                carrier_id: u32::from(carrier_slot),
                castle_id: castle_slot,
                mana: 321,
            }
        );

        assert!(first.economy_mut().submit_damage(castle_slot, 100, 77));
        assert!(first.economy_mut().submit_damage(castle_slot, 200, 77));
        assert_eq!(
            first.economy().castle().expect("castle").pending_damage,
            300
        );
        assert_eq!(first.economy_mut().resolve_damage(castle_slot), Some(false));
        assert_eq!(
            first.economy().castle().expect("castle").life,
            CASTLE_STAGE_ONE_LIFE - 300
        );
        assert_eq!(
            first.economy().events()[6],
            Level2EconomyEvent::DamageApplied {
                target_entity_index: castle_slot,
                attacker_entity_index: 77,
                damage: 300,
                lethal: false,
            }
        );
        let remaining_life = first.economy().castle().expect("castle").life;
        assert!(
            first
                .economy_mut()
                .submit_damage(castle_slot, remaining_life + 1, 88)
        );
        assert_eq!(first.economy_mut().resolve_damage(castle_slot), Some(true));
        let dead_castle = first.economy().castle().expect("castle");
        assert!(dead_castle.active);
        assert_eq!(dead_castle.killer_entity_index, 88);
        assert_eq!(
            first.economy().events()[7],
            Level2EconomyEvent::DamageApplied {
                target_entity_index: castle_slot,
                attacker_entity_index: 88,
                damage: remaining_life + 1,
                lethal: true,
            }
        );
        assert!(first.economy_mut().step_castle_action(castle_slot));
        assert_eq!(first.economy().castle().expect("castle").action, 6);
        assert!(first.economy_mut().step_castle_action(castle_slot));
        let reset_castle = first.economy().castle().expect("reset Castle");
        assert!(!reset_castle.active);
        assert_eq!((reset_castle.action, reset_castle.level), (4, 0));

        first.economy_mut().reclaim_inactive_slots();
        // The caller-side `sub_5FD00` pass releases the final Castle mana
        // after `sub_605E0`; that sphere remains active while the carrier is
        // still live, so two arena slots remain occupied.
        assert_eq!(first.economy().free_slot_count(), baseline_free_slots - 2);
        assert_eq!(first.economy().spheres().len(), 1);
        assert!(first.economy().spheres()[0].active);
        assert!(first.economy().carrier().is_some());
        assert!(first.economy().castle().is_none());

        let failed_hash_before = first.economy().reset_hash();
        first.economy_mut().free_slots.clear();
        assert!(
            first
                .economy_mut()
                .spawn_mana_sphere_from_death(43, Position::default(), 0, 0x9876, 12, 0)
                .is_none()
        );
        assert_eq!(first.economy().spheres().len(), 1);
        assert!(first.economy().spheres()[0].active);
        assert_eq!(first.economy().free_slot_count(), 0);
        assert_ne!(first.economy().reset_hash(), failed_hash_before);
        assert_eq!(first.economy().source_rng.get(&43), Some(&0x9876));

        assert_eq!(second.economy().reset_hash(), baseline_hash);
        first.reset();
        assert_eq!(first.economy().reset_hash(), baseline_hash);
        assert_eq!(first.economy().free_slot_count(), baseline_free_slots);
        assert!(first.economy().spheres().is_empty());
        assert!(first.economy().carrier().is_none());
        assert!(first.economy().castle().is_none());

        let fractional_ids = first.economy_mut().spawn_mana_spheres_from_death(
            44,
            Position::default(),
            0,
            0x2468,
            3_500,
            111,
            true,
        );
        assert_eq!(fractional_ids.len(), 3);
        let fractional_mana = fractional_ids
            .iter()
            .map(|id| {
                first
                    .economy()
                    .spheres()
                    .iter()
                    .find(|sphere| sphere.id == *id)
                    .expect("fractional sphere")
                    .mana
            })
            .collect::<Vec<_>>();
        assert_eq!(fractional_mana, vec![1_166, 1_166, 1_168]);
        assert_eq!(fractional_mana.iter().sum::<i32>(), 3_500);
        assert_eq!(first.economy().events().len(), 3);
        first.reset();
        assert_eq!(first.economy().reset_hash(), baseline_hash);

        let mut merge_session = inspection
            .level2_reset_session()
            .expect("merge projection session");
        let neutral_id = merge_session
            .economy_mut()
            .spawn_mana_sphere_from_death(45, Position::default(), 0, 0x1111, 100, 0)
            .expect("neutral sphere");
        let owned_id = merge_session
            .economy_mut()
            .spawn_mana_sphere_from_death(46, Position::default(), 0, 0x2222, 50, 111)
            .expect("owned sphere");
        merge_session.economy_mut().begin_turn();
        assert!(merge_session.economy_mut().merge_mana_spheres(
            u16::try_from(neutral_id).expect("sphere slot"),
            u16::try_from(owned_id).expect("sphere slot")
        ));
        let merged = merge_session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.id == neutral_id)
            .expect("merged target");
        assert_eq!(merged.owner_entity_index, 111);
        assert_eq!(merged.mana, 150);
        assert!(
            merge_session
                .economy()
                .spheres()
                .iter()
                .all(|sphere| sphere.id != owned_id)
        );
        assert_eq!(
            merge_session.economy().events(),
            &[Level2EconomyEvent::ManaSpheresMerged {
                target_sphere_id: neutral_id,
                source_sphere_id: owned_id,
                owner_entity_index: 111,
                mana: 50,
            }]
        );

        let mut carrier_gates = inspection
            .level2_reset_session()
            .expect("carrier gate session");
        let carrier_position = Position::default();
        let carrier_slot = carrier_gates
            .economy_mut()
            .spawn_mana_carrier(carrier_position, 111, 0)
            .expect("carrier allocation");
        let wrong_owner = carrier_gates
            .economy_mut()
            .spawn_mana_sphere_from_death(47, carrier_position, 0, 0x3333, 1, 112)
            .expect("wrong-owner sphere");
        assert!(!carrier_gates.economy_mut().absorb_sphere_into_carrier(
            u16::try_from(wrong_owner).expect("sphere slot"),
            carrier_slot
        ));
        let far_sphere = carrier_gates
            .economy_mut()
            .spawn_mana_sphere_from_death(
                48,
                Position {
                    x: 2_048,
                    ..carrier_position
                },
                0,
                0x4444,
                1,
                111,
            )
            .expect("far sphere");
        assert!(!carrier_gates.economy_mut().absorb_sphere_into_carrier(
            u16::try_from(far_sphere).expect("sphere slot"),
            carrier_slot
        ));
        let near_non_overlap = carrier_gates
            .economy_mut()
            .spawn_mana_sphere_from_death(
                49,
                Position {
                    x: 400,
                    ..carrier_position
                },
                0,
                0x5555,
                1,
                111,
            )
            .expect("near non-overlap sphere");
        assert!(!carrier_gates.economy_mut().absorb_sphere_into_carrier(
            u16::try_from(near_non_overlap).expect("sphere slot"),
            carrier_slot
        ));
        let near = carrier_gates
            .economy_mut()
            .spawn_mana_sphere_from_death(50, carrier_position, 0, 0x6666, 1, 111)
            .expect("near sphere");
        assert!(
            carrier_gates.economy_mut().absorb_sphere_into_carrier(
                u16::try_from(near).expect("sphere slot"),
                carrier_slot
            )
        );
        assert_eq!(
            carrier_gates
                .economy()
                .carrier()
                .expect("carrier")
                .target_sphere_id,
            0
        );

        let mut castle_lifecycle = inspection
            .level2_reset_session()
            .expect("Castle lifecycle session");
        let lifecycle_slot = castle_lifecycle
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("lifecycle Castle");
        let construction_anchor = Position {
            x: 3_072,
            y: 4_096,
            z: 512,
        };
        assert!(
            castle_lifecycle
                .economy_mut()
                .set_castle_anchor(lifecycle_slot, construction_anchor)
        );
        assert!(
            castle_lifecycle
                .economy_mut()
                .start_castle_construction(lifecycle_slot)
        );
        castle_lifecycle.economy_mut().free_slots.clear();
        assert!(
            !castle_lifecycle
                .economy_mut()
                .step_castle_action(lifecycle_slot)
        );
        let blocked = castle_lifecycle.economy().castle().expect("blocked Castle");
        assert_eq!(
            (blocked.action, blocked.build_state, blocked.level),
            (5, 0, 0)
        );
        castle_lifecycle.reset();
        assert_eq!(
            castle_lifecycle
                .economy()
                .map_chains()
                .linked_entity_indices(),
            expected_linked
        );
        let lifecycle_slot = castle_lifecycle
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("lifecycle Castle retry");
        assert!(
            castle_lifecycle
                .economy_mut()
                .set_castle_anchor(lifecycle_slot, construction_anchor)
        );
        assert!(
            castle_lifecycle
                .economy_mut()
                .start_castle_construction(lifecycle_slot)
        );
        assert!(
            castle_lifecycle
                .economy_mut()
                .step_castle_action(lifecycle_slot)
        );
        let construction = castle_lifecycle
            .economy()
            .castle()
            .expect("construction Castle");
        assert_eq!(
            (
                construction.construction_model,
                construction.construction_action,
                construction.construction_control_flags
            ),
            (42, 44, 1)
        );
        assert_eq!(castle_lifecycle.economy().castle_children().len(), 2);
        assert_eq!(
            castle_lifecycle.economy().castle_children()[0],
            Level2CastleChild {
                source_entity_index: construction.construction_slot,
                entity_rng: construction
                    .construction_slot
                    .wrapping_add(castle_lifecycle.economy().global_rng as u16),
                id_entity_index: 111,
                source_class: 10,
                parent_entity_index: lifecycle_slot,
                owner_entity_index: 111,
                player_entity_index: 0,
                position: construction_anchor,
                axis_position: Position::default(),
                model: 42,
                action: 44,
                mana: 0,
                max_mana: 0,
                life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                max_life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                speed: LEVEL2_CASTLE_CHILD_DEFAULT_SPEED,
                target_sphere_slot: 0,
                source_byte_3e: construction.construction_slot as u8,
                source_byte_3b: 1,
                cave_collision_cooldown: 1,
                source_dword_10: 0,
                source_word_2e: 0,
                construction_target_height: 0,
                source_subspell_index: 0,
                yaw: 0,
                roll: 0,
                control_flags: 1,
                chain_previous_slot: 0,
                chain_next_slot: 0,
                source_byte_43: 0,
                source_word_4a: 0,
                active: true,
            }
        );
        assert!(
            !castle_lifecycle
                .economy()
                .map_chains()
                .tracks(construction.construction_slot)
        );
        assert!(
            !castle_lifecycle
                .economy()
                .map_chains()
                .is_linked(construction.construction_slot)
        );
        let stage_child = castle_lifecycle
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.active && child.model == 79)
            .expect("source sub_613D0 stage controller");
        // `sub_613D0` owns this row through word32/word34; NewEvent leaves
        // parentId zero for the controller constructor.
        assert_eq!(stage_child.parent_entity_index, 0);
        assert_eq!(stage_child.id_entity_index, 111);
        assert_eq!(
            stage_child.position.z,
            castle_lifecycle
                .economy()
                .terrain
                .ground_height(construction.position)
                .saturating_add(384)
        );
        let lifecycle_castle = castle_lifecycle.economy().castle().expect("Castle");
        assert_eq!(lifecycle_castle.controller_chain_count, 1);
        assert_eq!(
            lifecycle_castle.controller_chain_head,
            lifecycle_castle.controller_chain_slots[0]
        );
        assert_eq!(
            lifecycle_castle.controller_chain_tail,
            lifecycle_castle.controller_chain_slots[0]
        );
        assert_eq!(stage_child.chain_previous_slot, lifecycle_slot);
        assert_eq!(stage_child.chain_next_slot, 0);
        assert_eq!(
            (stage_child.source_byte_43, stage_child.source_word_4a),
            (1, 1)
        );
        assert_eq!(lifecycle_castle.source_class, 3);
        assert_eq!(lifecycle_castle.source_model, 2);
        assert_eq!(lifecycle_castle.source_byte_38, 33);
        assert_eq!(lifecycle_castle.source_particle_parameter, 177);
        assert_eq!(
            castle_lifecycle.economy().player_castle_entity_index(),
            lifecycle_slot
        );
        assert_eq!(castle_lifecycle.economy().player_castle_stage(), 1);
        assert!(
            castle_lifecycle
                .economy_mut()
                .complete_castle_construction(lifecycle_slot)
        );
        assert!(
            castle_lifecycle
                .economy_mut()
                .step_castle_action(lifecycle_slot)
        );
        let built = castle_lifecycle.economy().castle().expect("built Castle");
        assert_eq!((built.action, built.build_state, built.level), (4, 0, 1));

        // Source sub_60810/sub_613D0 stage-two vector: the table updates
        // life/capacity and the controller anchor drops from +384 to +224.
        let mut stage_two = inspection
            .level2_reset_session()
            .expect("stage-two Castle session");
        let stage_two_slot = stage_two
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("stage-two Castle");
        assert!(
            stage_two
                .economy_mut()
                .start_castle_construction(stage_two_slot)
        );
        assert!(stage_two.economy_mut().step_castle_action(stage_two_slot));
        {
            let economy = stage_two.economy_mut();
            assert!(economy.set_castle_controller_coefficient(2, 0x2a));
            let castle = economy.castle.as_mut().expect("stage-two Castle");
            castle.action = 5;
            castle.build_state = 3;
            castle.level = 1;
            for child in &mut economy.castle_children {
                if child.parent_entity_index == stage_two_slot && child.model == 42 {
                    child.active = false;
                }
            }
        }
        assert!(stage_two.economy_mut().step_castle_action(stage_two_slot));
        let stage_two_castle = stage_two.economy().castle().expect("stage-two Castle");
        assert_eq!(
            (
                stage_two_castle.level,
                stage_two_castle.life,
                stage_two_castle.max_life,
                stage_two_castle.mana_capacity
            ),
            (2, 40_000, 40_000, 18_000)
        );
        let stage_two_child = stage_two
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.active && child.model == 79)
            .expect("stage-two controller");
        assert_eq!(stage_two_child.position.z, 224);
        assert_eq!(stage_two_child.source_byte_43, 0x2a);
        assert_eq!(stage_two_child.source_word_4a, 2);
        let stage_two_castle = stage_two.economy().castle().expect("stage-two Castle");
        assert_eq!(stage_two_castle.controller_chain_count, 4);
        assert_eq!(
            stage_two_castle.controller_chain_head,
            stage_two_castle.controller_chain_slots[0]
        );
        assert_eq!(
            stage_two_castle.controller_chain_tail,
            stage_two_castle.controller_chain_slots[3]
        );
        let stage_two_chain = stage_two
            .economy()
            .castle_children()
            .iter()
            .filter(|child| child.active && child.model == 79)
            .collect::<Vec<_>>();
        assert_eq!(stage_two_chain.len(), 4);
        for (index, child) in stage_two_chain.iter().enumerate() {
            let expected_previous = if index == 0 {
                stage_two_slot
            } else {
                stage_two_chain[index - 1].source_entity_index
            };
            let expected_next = stage_two_chain
                .get(index + 1)
                .map_or(0, |next| next.source_entity_index);
            assert_eq!(child.chain_previous_slot, expected_previous);
            assert_eq!(child.chain_next_slot, expected_next);
            assert_eq!((child.source_byte_43, child.source_word_4a), (0x2a, 2));
        }

        let mut defense_lifecycle = inspection
            .level2_reset_session()
            .expect("Castle defense lifecycle session");
        let defense_root = defense_lifecycle
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("defense Castle");
        assert!(
            defense_lifecycle
                .economy_mut()
                .set_castle_stage_coefficient(1, 128)
        );
        assert!(
            defense_lifecycle
                .economy_mut()
                .start_castle_construction(defense_root)
        );
        assert!(
            defense_lifecycle
                .economy_mut()
                .step_castle_action(defense_root)
        );
        assert!(
            defense_lifecycle
                .economy_mut()
                .start_castle_defense_child(defense_root)
        );
        assert!(
            defense_lifecycle
                .economy_mut()
                .step_castle_action(defense_root)
        );
        let defense = defense_lifecycle.economy().castle().expect("defense child");
        assert_eq!(
            (
                defense.defense_model,
                defense.defense_action,
                defense.defense_control_flags
            ),
            (41, 43, 0)
        );
        assert_eq!((defense.life, defense.max_life), (10_000, 10_000));
        assert_eq!(defense_lifecycle.economy().castle_children().len(), 3);
        assert_eq!(
            defense_lifecycle
                .economy()
                .castle_children()
                .iter()
                .filter(|child| child.active && child.model == 41)
                .count(),
            1
        );
        assert_eq!(
            defense_lifecycle
                .economy()
                .castle_children()
                .iter()
                .find(|child| child.active && child.model == 41)
                .expect("defense child")
                .position,
            defense_lifecycle
                .economy()
                .castle()
                .expect("defense Castle")
                .anchor_position
        );

        let mut action4_session = inspection
            .level2_reset_session()
            .expect("Castle action-4 session");
        let action4_slot = action4_session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("action-4 Castle");
        let action4_position = action4_session.economy().castle().expect("Castle").position;
        let action4_sphere = action4_session
            .economy_mut()
            .spawn_mana_sphere_from_death(51, action4_position, 0, 0x7777, 100, 111)
            .expect("Castle absorption sphere");
        action4_session.economy_mut().begin_turn();
        assert!(
            action4_session
                .economy_mut()
                .step_castle_action(action4_slot)
        );
        assert_eq!(action4_session.economy().castle().expect("Castle").mana, 0);
        assert_eq!(
            action4_session.economy().castle().expect("Castle").phase,
            (action4_slot as u8).wrapping_add(1)
        );
        assert!(
            action4_session
                .economy_mut()
                .step_castle_action(action4_slot)
        );
        assert_eq!(
            action4_session.economy().castle().expect("Castle").mana,
            100
        );
        assert!(
            !action4_session
                .economy()
                .spheres()
                .iter()
                .find(|sphere| sphere.id == action4_sphere)
                .expect("absorbed sphere")
                .active
        );
        // Source EndOfCastleProjectile calls sub_609E0 before the stage
        // maintenance chain. A non-lethal hit from the root's source ID sets
        // the player byte-195 marker and requests state-0 construction.
        let mut damage_session = inspection
            .level2_reset_session()
            .expect("Castle damage session");
        let damage_slot = damage_session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("damage Castle");
        assert!(
            damage_session
                .economy_mut()
                .submit_damage(damage_slot, 1, 111)
        );
        assert!(damage_session.economy_mut().step_castle_action(damage_slot));
        let damage_transition = damage_session.economy().castle().expect("Castle");
        assert_eq!(
            (damage_transition.action, damage_transition.build_state),
            (5, 0)
        );
        assert_eq!(damage_session.economy().player_castle_aux_flags(), 4);
        assert_ne!(damage_transition.control_flags & 0x40, 0);

        // Source sub_5FD00 releases only overflow above stage capacity. The
        // anchor is deliberately separated from the root to prove the later
        // source-order model-39 scan does not immediately reclaim that sphere.
        let mut release_session = inspection
            .level2_reset_session()
            .expect("Castle release session");
        let release_slot = release_session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("release Castle");
        let release_anchor = Position {
            x: 8_192,
            y: 8_192,
            z: 0,
        };
        assert!(
            release_session
                .economy_mut()
                .set_castle_anchor(release_slot, release_anchor)
        );
        {
            let economy = release_session.economy_mut();
            economy.player_mana = 0;
            let castle = economy.castle.as_mut().expect("release Castle");
            castle.mana = castle.mana_capacity + 1_000;
            // Force the source's even-phase maintenance branch for this
            // vector while retaining the slot-seeded default in production.
            castle.phase &= !1;
        }
        assert!(
            release_session
                .economy_mut()
                .step_castle_action(release_slot)
        );
        let release_castle = release_session.economy().castle().expect("release Castle");
        assert_eq!(release_castle.mana, release_castle.mana_capacity);
        assert!(
            release_session
                .economy()
                .spheres()
                .iter()
                .any(|sphere| sphere.active
                    && sphere.owner_entity_index == 111
                    && sphere.mana == 1_000)
        );

        // When the requested source fraction is larger than the remaining
        // arena, sub_5FD00 recomputes the share from the committed count; it
        // does not silently lose the unreleased remainder.
        let mut partial_release = inspection
            .level2_reset_session()
            .expect("partial Castle release session");
        let partial_slot = partial_release
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("partial release Castle");
        {
            let economy = partial_release.economy_mut();
            economy.player_mana = 0;
            economy.free_slots.truncate(2);
            let castle = economy.castle.as_mut().expect("partial release Castle");
            castle.mana = castle.mana_capacity + 3_500;
            castle.phase &= !1;
        }
        assert!(
            partial_release
                .economy_mut()
                .step_castle_action(partial_slot)
        );
        let partial = partial_release
            .economy()
            .spheres()
            .iter()
            .filter(|sphere| sphere.active && sphere.owner_entity_index == 111)
            .map(|sphere| sphere.mana)
            .collect::<Vec<_>>();
        assert_eq!(partial, vec![1_750, 1_750]);
        assert_eq!(partial.iter().sum::<i32>(), 3_500);
        action4_session
            .economy_mut()
            .castle
            .as_mut()
            .expect("Castle")
            .timer = 2;
        assert!(
            action4_session
                .economy_mut()
                .step_castle_action(action4_slot)
        );
        assert_eq!(action4_session.economy().castle().expect("Castle").timer, 1);
        assert!(
            action4_session
                .economy_mut()
                .step_castle_action(action4_slot)
        );
        let timer_transition = action4_session.economy().castle().expect("Castle");
        assert_eq!(
            (
                timer_transition.action,
                timer_transition.build_state,
                timer_transition.timer
            ),
            (5, 3, 0)
        );

        // Source sub_605E0/sub_5FCA0 lowers maxMana by ten percent before
        // releasing overflow, then restores the next stage's exact table
        // values. The stage transition itself is not blocked when the arena
        // has no free slots for the optional overflow spheres.
        let mut teardown = inspection
            .level2_reset_session()
            .expect("Castle teardown session");
        let teardown_slot = teardown
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("teardown Castle");
        assert!(
            teardown
                .economy_mut()
                .start_castle_construction(teardown_slot)
        );
        assert!(teardown.economy_mut().step_castle_action(teardown_slot));
        {
            let economy = teardown.economy_mut();
            economy.set_player_spell_cleanup_context(1, 4, 777);
            economy.set_player_spell_holder_counters(17, 9);
            let castle = economy.castle.as_mut().expect("teardown Castle");
            castle.action = 6;
            castle.level = 2;
            castle.life = -10_000;
            castle.max_life = 40_000;
            castle.mana = 20_000;
            castle.mana_capacity = 18_000;
        }
        let terminal_cursor_rows = teardown
            .economy()
            .live_arena_cursor_rows()
            .into_iter()
            .filter(|row| {
                matches!(
                    row.kind,
                    Level2DynamicArenaKind::CastleChild | Level2DynamicArenaKind::CastleController
                )
            })
            .map(|row| (row.source_slot, row.generation, row.kind))
            .collect::<Vec<_>>();
        let mut expected_terminal_cursor_rows = teardown
            .economy()
            .castle_children()
            .iter()
            .filter(|child| child.active)
            .filter_map(|child| {
                let kind = match child.model {
                    42 => Level2DynamicArenaKind::CastleChild,
                    79 => Level2DynamicArenaKind::CastleController,
                    _ => return None,
                };
                Some((
                    child.source_entity_index,
                    teardown
                        .economy()
                        .slot_generation(child.source_entity_index),
                    kind,
                ))
            })
            .collect::<Vec<_>>();
        expected_terminal_cursor_rows.sort_unstable_by_key(|row| row.0);
        assert_eq!(terminal_cursor_rows, expected_terminal_cursor_rows);
        assert!(teardown.economy_mut().step_castle_action(teardown_slot));
        let downgraded = teardown.economy().castle().expect("downgraded Castle");
        assert!(downgraded.active);
        assert_eq!(
            (
                downgraded.action,
                downgraded.build_state,
                downgraded.level,
                downgraded.life,
                downgraded.max_life,
                downgraded.mana,
                downgraded.mana_capacity,
                downgraded.timer
            ),
            // `sub_5FD00` uses the floor share for each allocated sphere;
            // its two-unit quotient remainder stays in the Castle payload.
            (4, 0, 1, 10_000, 20_000, 8_502, 8_500, 5)
        );
        assert_eq!(
            (
                downgraded.shift_pitch,
                downgraded.shift_roll,
                downgraded.shift_yaw,
                downgraded.shift_fov
            ),
            (1_664, 1_664, -8_192, 0x4000)
        );
        assert_eq!(
            teardown.economy().player_castle_entity_index(),
            teardown_slot
        );
        assert_eq!(teardown.economy().player_castle_stage(), 1);
        assert_eq!(teardown.economy().player_spell_slot_2(), 777);
        assert_eq!(teardown.economy().player_spell_holder_action(), 8);
        assert_eq!(
            (downgraded.source_word_2e, downgraded.source_word_30),
            (0, 5)
        );
        assert!(teardown.economy().events().contains(
            &Level2EconomyEvent::CastleDestructionSound {
                source_entity_index: teardown_slot,
                sound_id: 30,
            }
        ));
        assert!(
            teardown
                .economy()
                .events()
                .contains(&Level2EconomyEvent::CastleStageDestroyed {
                    source_entity_index: teardown_slot,
                    previous_level: 2,
                    next_level: 1,
                })
        );
        let callback_child_rows = teardown
            .economy()
            .castle_children()
            .iter()
            .filter(|child| child.active)
            .map(|child| {
                (
                    child.source_entity_index,
                    child.source_class,
                    child.model,
                    child.action,
                    child.parent_entity_index,
                    child.chain_previous_slot,
                    child.chain_next_slot,
                )
            })
            .collect::<Vec<_>>();
        let model3_slot = callback_child_rows
            .iter()
            .find(|row| (row.1, row.2, row.3) == (3, 3, 9))
            .map(|row| row.0)
            .expect("source model-3 callback row");
        let model42_slot = callback_child_rows
            .iter()
            .find(|row| (row.1, row.2, row.3) == (10, 42, 44))
            .map(|row| row.0)
            .expect("source model-42 callback row");
        assert_eq!(
            callback_child_rows,
            vec![
                (model42_slot, 10, 42, 44, teardown_slot, 0, 0),
                (
                    downgraded.controller_chain_slots[0],
                    10,
                    79,
                    0x56,
                    0,
                    teardown_slot,
                    0
                ),
                (model3_slot, 3, 3, 9, 0, 0, 0),
            ]
        );

        teardown.select_level2_campaign_entry_stage();
        teardown.step_outer_turn(PlayerCommand::default(), 0);
        assert!(teardown.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Castle { source_slot, .. }
                    if *source_slot == teardown_slot
            )
        }));
        let terminal_dispatch_rows = teardown
            .dispatch_events()
            .iter()
            .filter_map(|event| match event {
                Level2DispatchEvent::DynamicArena {
                    source_slot, kind, ..
                } if matches!(
                    kind,
                    Level2DynamicArenaKind::CastleChild | Level2DynamicArenaKind::CastleController
                ) =>
                {
                    Some((*source_slot, *kind))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            terminal_dispatch_rows,
            vec![
                (model42_slot, Level2DynamicArenaKind::CastleChild),
                (
                    downgraded.controller_chain_slots[0],
                    Level2DynamicArenaKind::CastleController
                ),
            ]
        );
        let live_cursor_rows = teardown.economy().live_arena_cursor_rows();
        let terminal_child = terminal_cursor_rows
            .iter()
            .find(|(_, _, kind)| *kind == Level2DynamicArenaKind::CastleChild)
            .copied()
            .expect("terminal construction row");
        let terminal_controller = terminal_cursor_rows
            .iter()
            .find(|(_, _, kind)| *kind == Level2DynamicArenaKind::CastleController)
            .copied()
            .expect("terminal controller row");
        let terminal_child_live = live_cursor_rows
            .iter()
            .find(|row| row.source_slot == terminal_child.0)
            .copied()
            .expect("stage teardown reuses the construction slot");
        assert_eq!(terminal_child_live.kind, terminal_child.2);
        assert_eq!(terminal_child_live.generation, terminal_child.1);
        assert!(!teardown.economy().map_chains().is_linked(terminal_child.0));
        assert!(!teardown.economy().free_slots.contains(&terminal_child.0));
        let terminal_controller_live = live_cursor_rows
            .iter()
            .find(|row| row.source_slot == terminal_controller.0)
            .copied()
            .expect("stage teardown reuses the controller slot");
        // The source stage transition may immediately repopulate this slot
        // with the next live model-79 controller; either that controller or a
        // source-order replacement is valid, but the row must be new, linked,
        // and outside the free stack.
        assert_eq!(
            terminal_controller_live.kind,
            Level2DynamicArenaKind::CastleController
        );
        assert_ne!(terminal_controller_live.generation, terminal_controller.1);
        assert!(
            teardown
                .economy()
                .map_chains()
                .is_linked(terminal_controller.0)
        );
        let terminal_post_turn_rows = terminal_cursor_rows
            .iter()
            .map(|(source_slot, generation, _)| {
                (
                    *source_slot,
                    *generation,
                    live_cursor_rows
                        .iter()
                        .find(|row| row.source_slot == *source_slot)
                        .map(|row| (row.source_slot, row.generation, row.kind)),
                    teardown.economy().map_chains().is_linked(*source_slot),
                    teardown.economy().free_slots.contains(source_slot),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            terminal_post_turn_rows,
            vec![
                (
                    terminal_child.0,
                    terminal_child.1,
                    Some((
                        terminal_child_live.source_slot,
                        terminal_child_live.generation,
                        terminal_child_live.kind,
                    )),
                    false,
                    false,
                ),
                (
                    terminal_controller.0,
                    terminal_controller.1,
                    Some((
                        terminal_controller_live.source_slot,
                        terminal_controller_live.generation,
                        terminal_controller_live.kind,
                    )),
                    true,
                    false,
                ),
            ]
        );
        assert_eq!(
            teardown
                .economy()
                .castle_children()
                .iter()
                .filter(|child| child.active)
                .map(|child| {
                    (
                        child.source_entity_index,
                        child.source_class,
                        child.model,
                        child.action,
                        child.parent_entity_index,
                        child.chain_previous_slot,
                        child.chain_next_slot,
                    )
                })
                .collect::<Vec<_>>(),
            vec![
                (model42_slot, 10, 42, 44, teardown_slot, 0, 0),
                (
                    downgraded.controller_chain_slots[0],
                    10,
                    79,
                    0x56,
                    0,
                    teardown_slot,
                    0,
                ),
                (model3_slot, 3, 3, 9, 0, 0, 0),
            ]
        );

        // A final-stage teardown also completes with an exhausted arena and
        // clears the player Castle link plus the model-1 spell holder.
        let mut final_teardown = inspection
            .level2_reset_session()
            .expect("final Castle teardown session");
        let final_baseline_hash = final_teardown.economy().reset_hash();
        let final_baseline_holder = final_teardown.economy().player_spell_slot_2();
        let final_slot = final_teardown
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("final Castle");
        {
            let economy = final_teardown.economy_mut();
            economy.set_player_spell_cleanup_context(1, 4, 888);
            let castle = economy.castle.as_mut().expect("final Castle");
            castle.action = 6;
            castle.level = 1;
            castle.life = -1;
            castle.max_life = 20_000;
        }
        assert!(final_teardown.economy_mut().step_castle_action(final_slot));
        let destroyed = final_teardown.economy().castle().expect("destroyed Castle");
        assert!(!destroyed.active);
        assert_eq!((destroyed.source_word_2e, destroyed.source_word_30), (0, 5));
        assert_eq!(final_teardown.economy().player_castle_entity_index(), 0);
        assert_eq!(final_teardown.economy().player_castle_stage(), 0);
        assert_eq!(final_teardown.economy().player_spell_slot_2(), 0);
        assert!(
            final_teardown
                .economy()
                .events()
                .contains(&Level2EconomyEvent::CastleDestroyed {
                    source_entity_index: final_slot,
                    killer_entity_index: 0,
                })
        );
        final_teardown.economy_mut().reset();
        assert_eq!(final_teardown.economy().reset_hash(), final_baseline_hash);
        assert_eq!(
            final_teardown.economy().player_spell_slot_2(),
            final_baseline_holder
        );
    }

    #[test]
    fn level_two_economy_conformance_death_split_and_merge_vectors() {
        // Pinned remc2 6806c72, EventsFunctions.cpp:26867-26915
        // (TransformEntityToManaSphere_36BA0): fraction is clamp(mana / 1000,
        // 1, 16), each sphere gets the floor share, and the final sphere gets
        // the exact remainder. The source also advances the sphere-local RNG
        // twice before deriving yaw and speed and clears the dying owner.
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut split = inspection
            .level2_reset_session()
            .expect("economy split session");
        // InitialiseSpells_54A50 reserves the live family-0..3 holder rows
        // before the economy's first ordinary allocation.  Derive the
        // remaining stack from the inspected inactive census rather than
        // baking the pre-holder baseline into this conformance vector.
        let holder_slots = split
            .shared_spells()
            .dynamic_pickups()
            .iter()
            .map(|holder| holder.index)
            .collect::<Vec<_>>();
        let mut expected_free_stack = inspection
            .entities()
            .iter()
            .filter(|entity| {
                entity.index != 0 && !entity.active && !holder_slots.contains(&entity.index)
            })
            .map(|entity| entity.index)
            .collect::<Vec<_>>();
        expected_free_stack.sort_unstable_by(|left, right| right.cmp(left));
        assert_eq!(split.economy().free_slots, expected_free_stack);
        let expected_slots = split
            .economy()
            .free_slots
            .iter()
            .rev()
            .take(16)
            .copied()
            .collect::<Vec<_>>();
        let first_slot = expected_slots[0];
        let first_seed = first_slot.wrapping_add(split.runtime().random_state as u16);
        let first_rng = level2_next_rng16(first_seed);
        let second_rng = level2_next_rng16(first_rng);
        let baseline_hash = split.economy().reset_hash();
        split.economy_mut().begin_turn();
        let sphere_ids = split.economy_mut().spawn_mana_spheres_from_death(
            42,
            Position {
                x: 384,
                y: 62_080,
                z: 1_024,
            },
            0x120,
            0x4321,
            17_000,
            121,
            true,
        );
        assert_eq!(
            sphere_ids,
            expected_slots
                .iter()
                .map(|slot| u32::from(*slot))
                .collect::<Vec<_>>()
        );
        assert_eq!(split.economy().source_rng.get(&42), Some(&0x4321));
        assert_eq!(split.economy().source_owner(42), 0);
        assert_eq!(split.economy().events().len(), 16);
        let first = split
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.id == sphere_ids[0])
            .expect("first spawned sphere");
        assert_eq!(first.entity_rng, second_rng);
        assert_eq!(
            first.yaw,
            i16::try_from((i32::from(first_rng % 0x71) + i32::from(0x120_i16) - 56) & 0x07ff)
                .expect("source yaw fits")
        );
        assert_eq!(first.mana, 1_062);
        assert_eq!(
            split
                .economy()
                .spheres()
                .iter()
                .map(|sphere| sphere.mana)
                .collect::<Vec<_>>(),
            [vec![1_062; 15], vec![1_070]].concat()
        );
        for (event, &sphere_id) in split.economy().events().iter().zip(&sphere_ids) {
            assert_eq!(
                *event,
                Level2EconomyEvent::ManaSphereSpawned {
                    sphere_id,
                    source_entity_index: 42,
                    mana: split
                        .economy()
                        .spheres()
                        .iter()
                        .find(|sphere| sphere.id == sphere_id)
                        .expect("spawned sphere")
                        .mana,
                }
            );
        }
        split.reset();
        assert_eq!(split.economy().reset_hash(), baseline_hash);
        assert!(split.economy().spheres().is_empty());
        let mut invalid_merge = inspection
            .level2_reset_session()
            .expect("invalid merge session");
        invalid_merge.economy_mut().begin_turn();
        assert!(!invalid_merge.economy_mut().merge_mana_spheres(0, 1));
        assert!(invalid_merge.economy().events().is_empty());

        // Pinned remc2 6806c72, EventsFunctions.cpp:26919-26997
        // (sub_36D50): neutral ownership adopts the other side, class-10
        // owners yield to non-class-10 owners, and non-class-10 ties choose
        // the source side when maxMana is equal.
        struct MergeVector {
            target_owner: u16,
            source_owner: u16,
            target_class: u8,
            source_class: u8,
            target_max_mana: i32,
            source_max_mana: i32,
            expected_owner: u16,
        }
        let vectors = [
            MergeVector {
                target_owner: 0,
                source_owner: 201,
                target_class: 0,
                source_class: 3,
                target_max_mana: 0,
                source_max_mana: 0,
                expected_owner: 201,
            },
            MergeVector {
                target_owner: 200,
                source_owner: 201,
                target_class: 10,
                source_class: 3,
                target_max_mana: 900,
                source_max_mana: 100,
                expected_owner: 201,
            },
            MergeVector {
                target_owner: 200,
                source_owner: 201,
                target_class: 3,
                source_class: 10,
                target_max_mana: 900,
                source_max_mana: 100,
                expected_owner: 200,
            },
            MergeVector {
                target_owner: 200,
                source_owner: 201,
                target_class: 10,
                source_class: 10,
                target_max_mana: 100,
                source_max_mana: 900,
                expected_owner: 200,
            },
            MergeVector {
                target_owner: 200,
                source_owner: 201,
                target_class: 3,
                source_class: 3,
                target_max_mana: 100,
                source_max_mana: 100,
                expected_owner: 201,
            },
        ];
        for vector in vectors {
            let mut session = inspection
                .level2_reset_session()
                .expect("merge vector session");
            session
                .economy_mut()
                .owner_class
                .insert(vector.target_owner, vector.target_class);
            session
                .economy_mut()
                .owner_class
                .insert(vector.source_owner, vector.source_class);
            session
                .economy_mut()
                .set_owner_max_mana(vector.target_owner, vector.target_max_mana);
            session
                .economy_mut()
                .set_owner_max_mana(vector.source_owner, vector.source_max_mana);
            let target_id = session
                .economy_mut()
                .spawn_mana_sphere_from_death(
                    45,
                    Position::default(),
                    0,
                    0x1111,
                    100,
                    vector.target_owner,
                )
                .expect("merge target");
            let source_id = session
                .economy_mut()
                .spawn_mana_sphere_from_death(
                    46,
                    Position::default(),
                    0,
                    0x2222,
                    50,
                    vector.source_owner,
                )
                .expect("merge source");
            session.economy_mut().begin_turn();
            assert!(session.economy_mut().merge_mana_spheres(
                u16::try_from(target_id).expect("target slot"),
                u16::try_from(source_id).expect("source slot"),
            ));
            let target = session
                .economy()
                .spheres()
                .iter()
                .find(|sphere| sphere.id == target_id)
                .expect("merged target");
            assert_eq!(
                (target.owner_entity_index, target.mana),
                (vector.expected_owner, 150)
            );
            assert!(
                session
                    .economy()
                    .spheres()
                    .iter()
                    .all(|sphere| sphere.id != source_id)
            );
            assert_eq!(
                session.economy().events(),
                &[Level2EconomyEvent::ManaSpheresMerged {
                    target_sphere_id: target_id,
                    source_sphere_id: source_id,
                    owner_entity_index: vector.expected_owner,
                    mana: 50,
                }]
            );
        }
    }

    #[test]
    fn level_two_mana_sphere_merge_reclaims_source_slot_immediately() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("Level 2 merge reclaim session");
        let baseline_free_slots = session.economy().free_slot_count();
        let target_slot = session
            .economy_mut()
            .spawn_mana_sphere_from_death(903, Position::default(), 0, 0x5678, 100, 111)
            .expect("target sphere allocation");
        let source_slot = session
            .economy_mut()
            .spawn_mana_sphere_from_death(904, Position::default(), 0, 0x6789, 50, 112)
            .expect("source sphere allocation");
        assert_eq!(session.economy().free_slot_count(), baseline_free_slots - 2);
        session.economy_mut().begin_turn();
        assert!(session.economy_mut().merge_mana_spheres(
            u16::try_from(target_slot).expect("target slot fits"),
            u16::try_from(source_slot).expect("source slot fits"),
        ));
        assert_eq!(
            session.economy().free_slot_count(),
            baseline_free_slots - 1,
            "sub_57F20 makes the absorbed source slot available immediately"
        );
        assert!(
            !session
                .economy()
                .map_chains()
                .linked_entity_indices()
                .contains(&u16::try_from(source_slot).expect("source slot fits"))
        );
        session.economy_mut().reclaim_inactive_slots();
        assert_eq!(
            session.economy().free_slot_count(),
            baseline_free_slots - 1,
            "reclaim pass must not double-release an already absorbed slot"
        );
    }

    #[test]
    fn level_two_economy_conformance_addballon_carrier_vectors() {
        // Pinned remc2 6806c72, EventsFunctions.cpp:61802-61904
        // (AddBallon_60AB0): model-39 collection requires matching owner,
        // wrapped attraction distance, and the strict source AABB before the
        // carrier receives the payload and clears word96 on the sphere.
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let carrier_position = Position::default();
        let mut session = inspection
            .level2_reset_session()
            .expect("carrier vector session");
        let carrier_slot = session
            .economy_mut()
            .spawn_mana_carrier(carrier_position, 111, 0)
            .expect("carrier allocation");
        let wrong_owner = session
            .economy_mut()
            .spawn_mana_sphere_from_death(47, carrier_position, 0, 0x3333, 1, 112)
            .expect("wrong-owner sphere");
        let far = session
            .economy_mut()
            .spawn_mana_sphere_from_death(
                48,
                Position {
                    x: 2_048,
                    ..carrier_position
                },
                0,
                0x4444,
                1,
                111,
            )
            .expect("far sphere");
        let near_but_outside_aabb = session
            .economy_mut()
            .spawn_mana_sphere_from_death(
                49,
                Position {
                    x: 400,
                    ..carrier_position
                },
                0,
                0x5555,
                1,
                111,
            )
            .expect("near non-overlap sphere");
        let valid = session
            .economy_mut()
            .spawn_mana_sphere_from_death(50, carrier_position, 0, 0x6666, 321, 111)
            .expect("valid sphere");
        session.economy_mut().begin_turn();
        for rejected in [wrong_owner, far, near_but_outside_aabb] {
            assert!(!session.economy_mut().absorb_sphere_into_carrier(
                u16::try_from(rejected).expect("sphere slot"),
                carrier_slot,
            ));
        }
        assert!(
            session.economy_mut().absorb_sphere_into_carrier(
                u16::try_from(valid).expect("sphere slot"),
                carrier_slot,
            )
        );
        let carrier = session.economy().carrier().expect("active carrier");
        assert_eq!((carrier.carried_mana, carrier.target_sphere_id), (321, 0));
        assert_eq!(
            session.economy().events(),
            &[Level2EconomyEvent::ManaSphereCollected {
                sphere_id: valid,
                carrier_id: u32::from(carrier_slot),
                mana: 321,
            }]
        );
        assert!(
            !session
                .economy()
                .spheres()
                .iter()
                .find(|sphere| sphere.id == valid)
                .expect("valid sphere")
                .active
        );
        assert_eq!(
            session
                .economy()
                .spheres()
                .iter()
                .filter(|sphere| sphere.active)
                .count(),
            3
        );
    }

    #[test]
    fn level_two_economy_conformance_castle_action_vectors_and_allocator_fallback() {
        // Pinned remc2 6806c72, Events.cpp:2937-2964 dispatches action 4/6
        // through the Castle helpers; EventsFunctions.cpp:61162-61278 binds
        // the construction state machine and sub_5FCA0's free-arena gate.
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let footprint = CastleBuildFootprint::from_owner_cells(1, 8, 8, &[0xff; 128])
            .expect("source BUILD vector");
        let projectile = Position {
            x: 0x13_45,
            y: 0x56_78,
            z: 1_234,
        };
        let mut production = inspection
            .level2_reset_session()
            .expect("production Castle session");
        let castle_slot = production
            .economy_mut()
            .build_castle_with_footprint(projectile, 111, &footprint)
            .expect("production Castle allocation");
        let castle = production.economy().castle().expect("production Castle");
        assert_eq!((castle.position.x, castle.position.y), (0x14_00, 0x56_00));
        assert_eq!(
            (castle.anchor_position.x, castle.anchor_position.y),
            (0x14_00, 0x56_00)
        );
        production.economy_mut().begin_turn();
        assert!(production.economy_mut().step_castle_action(castle_slot));
        let construction = production
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.active && child.model == 42)
            .expect("production construction child");
        assert_eq!(construction.position, castle.anchor_position);
        assert_eq!(
            production.economy().events()[0],
            Level2EconomyEvent::CastleConstructionSound {
                source_entity_index: castle_slot,
                sound_id: 10,
            }
        );

        // sub_5FF50/EndOfCastleProjectile's action-4 even-phase branch absorbs
        // an owned sphere before the source phase byte advances.
        let mut action4 = inspection
            .level2_reset_session()
            .expect("action-4 Castle session");
        let action4_slot = action4
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("action-4 Castle");
        let action4_position = action4.economy().castle().expect("Castle").position;
        let action4_sphere = action4
            .economy_mut()
            .spawn_mana_sphere_from_death(51, action4_position, 0, 0x7777, 100, 111)
            .expect("action-4 sphere");
        action4
            .economy_mut()
            .castle
            .as_mut()
            .expect("action-4 Castle")
            .phase = 0;
        action4.economy_mut().begin_turn();
        assert!(action4.economy_mut().step_castle_action(action4_slot));
        assert_eq!(action4.economy().castle().expect("Castle").mana, 100);
        assert_eq!(action4.economy().castle().expect("Castle").phase, 1);
        assert!(
            !action4
                .economy()
                .spheres()
                .iter()
                .find(|sphere| sphere.id == action4_sphere)
                .expect("action-4 sphere")
                .active
        );
        assert_eq!(
            action4.economy().events(),
            &[Level2EconomyEvent::ManaSphereCollected {
                sphere_id: action4_sphere,
                carrier_id: u32::from(action4_slot),
                mana: 100,
            }]
        );

        // With a free slot, sub_5FCA0 emits destruction sound before the
        // stage-destroyed and final Castle-destroyed events. This is a source
        // ordering vector, not just a final-state assertion.
        let mut teardown = inspection
            .level2_reset_session()
            .expect("action-6 Castle session");
        let teardown_slot = teardown
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("action-6 Castle");
        {
            let castle = teardown.economy_mut().castle.as_mut().expect("Castle");
            castle.action = 6;
            castle.level = 1;
            castle.life = -1;
            castle.max_life = 20_000;
        }
        teardown.economy_mut().begin_turn();
        assert!(teardown.economy_mut().step_castle_action(teardown_slot));
        assert_eq!(
            teardown.economy().events(),
            &[
                Level2EconomyEvent::CastleDestructionSound {
                    source_entity_index: teardown_slot,
                    sound_id: 30,
                },
                Level2EconomyEvent::CastleStageDestroyed {
                    source_entity_index: teardown_slot,
                    previous_level: 1,
                    next_level: 0,
                },
                Level2EconomyEvent::CastleDestroyed {
                    source_entity_index: teardown_slot,
                    killer_entity_index: 0,
                },
            ]
        );
        assert!(
            !teardown
                .economy()
                .castle()
                .expect("destroyed Castle")
                .active
        );

        // The source writes the dying source RNG/owner before allocation and
        // leaves no partial event when the source arena is exhausted.
        let mut exhausted = inspection
            .level2_reset_session()
            .expect("allocator fallback session");
        exhausted.economy_mut().free_slots.clear();
        exhausted.economy_mut().begin_turn();
        assert!(
            exhausted
                .economy_mut()
                .spawn_mana_sphere_from_death(52, Position::default(), 0, 0x9876, 12, 111)
                .is_none()
        );
        assert_eq!(exhausted.economy().source_rng.get(&52), Some(&0x9876));
        assert_eq!(exhausted.economy().source_owner(52), 0);
        assert!(exhausted.economy().events().is_empty());

        // sub_5FCA0's no-free-slot branch resets action to 4 and does not
        // commit a destruction sound or stage transition.
        let mut blocked = inspection
            .level2_reset_session()
            .expect("blocked action-6 session");
        let blocked_slot = blocked
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("blocked Castle");
        {
            let economy = blocked.economy_mut();
            economy.free_slots.clear();
            let castle = economy.castle.as_mut().expect("blocked Castle");
            castle.action = 6;
            castle.level = 1;
        }
        blocked.economy_mut().begin_turn();
        assert!(blocked.economy_mut().step_castle_action(blocked_slot));
        let blocked_castle = blocked.economy().castle().expect("blocked Castle");
        assert_eq!((blocked_castle.action, blocked_castle.level), (4, 1));
        assert!(blocked.economy().events().is_empty());
    }

    #[test]
    fn level_two_castle_stage_controller_matches_source() {
        let package = level_two_fixture_path();
        let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
            .expect("qualified Level 2 gameplay component");
        assert_eq!(inspection.metadata().vga_resolution_selector, 1);
        assert_eq!(
            LEVEL2_CASTLE_MODEL41_MSPRD_RECTS,
            [
                (0, 0),
                (16, 11),
                (16, 11),
                (16, 9),
                (16, 9),
                (16, 11),
                (1, 0),
                (1, 0)
            ]
        );
        let expected_dimensions = [
            (1, (8, 8)),
            (2, (21, 21)),
            (3, (21, 21)),
            (4, (35, 35)),
            (5, (35, 35)),
            (6, (48, 48)),
            (7, (48, 48)),
        ];
        assert_eq!(inspection.level2_castle_stage_footprints.len(), 7);
        for (stage, (width, height)) in expected_dimensions {
            let footprint = inspection
                .level2_castle_stage_footprints
                .get(&stage)
                .expect("every Castle stage footprint is installed");
            assert_eq!(
                (footprint.index, footprint.width, footprint.height),
                (stage, width, height)
            );
            assert_eq!(
                footprint.cells().len(),
                usize::from(width) * usize::from(height)
            );
        }

        // The stage controller is reached through the ordinary Level 2
        // source pass, using the footprint installed by the qualified cache.
        let stage_one = inspection
            .level2_castle_stage_footprints
            .get(&1)
            .expect("stage-one footprint")
            .clone();
        let mut controller = inspection
            .level2_reset_session()
            .expect("cache-loaded controller session");
        controller.select_level2_campaign_entry_stage();
        let castle_slot = controller
            .economy_mut()
            .build_castle_with_footprint(
                Position {
                    x: 0x20_00,
                    y: 0x30_00,
                    z: 0,
                },
                111,
                &stage_one,
            )
            .expect("ordinary Castle allocation");
        controller.step_outer_turn(PlayerCommand::default(), 0);
        let castle = controller.economy().castle().expect("live Castle");
        assert_eq!(castle.source_entity_index, castle_slot);
        assert_eq!((castle.level, castle.controller_chain_count), (1, 1));
        let stage_controller_slot = controller
            .dispatch_events()
            .iter()
            .find_map(|event| match event {
                Level2DispatchEvent::DynamicArena {
                    source_slot,
                    kind: Level2DynamicArenaKind::CastleController,
                    ..
                } => Some(*source_slot),
                _ => None,
            })
            .expect("ordinary source-pass stage controller");
        assert_eq!(
            stage_controller_slot,
            controller
                .economy()
                .castle()
                .expect("Castle after controller dispatch")
                .controller_chain_slots[0]
        );
        let first_controller = controller
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == stage_controller_slot)
            .copied()
            .expect("live model-79 controller after first callback");
        assert_eq!(
            (first_controller.model, first_controller.action),
            (79, 0x56)
        );
        assert_eq!(
            (first_controller.life, first_controller.max_life),
            (100_000, 100_000)
        );
        assert_eq!(first_controller.cave_collision_cooldown, 1);
        assert_eq!(
            first_controller.source_byte_3e,
            (stage_controller_slot as u8).wrapping_add(1)
        );
        assert_eq!(
            first_controller.axis_position.x,
            first_controller.position.x
        );
        assert_eq!(
            first_controller.axis_position.y,
            first_controller.position.y
        );
        let ground = controller
            .economy()
            .terrain
            .ground_height(first_controller.axis_position);
        let base_z = ground.saturating_add(384);
        assert_eq!(
            first_controller.position.z,
            if first_controller.axis_position.z < base_z {
                base_z
            } else if stage_controller_slot as u8 & 8 != 0 {
                first_controller.axis_position.z.saturating_add(6)
            } else {
                first_controller.axis_position.z.saturating_sub(6)
            }
        );
        assert!(
            controller
                .economy()
                .map_chains()
                .is_linked(stage_controller_slot)
        );
        assert!(
            !controller
                .economy()
                .free_slots
                .contains(&stage_controller_slot)
        );
        let second_rng = first_controller
            .entity_rng
            .wrapping_mul(9_377)
            .wrapping_add(9_439);
        assert!(
            controller
                .economy_mut()
                .step_castle_child_terminal_slot(stage_controller_slot)
        );
        let second_controller = controller
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == stage_controller_slot)
            .copied()
            .expect("live model-79 controller after second callback");
        assert_eq!(second_controller.entity_rng, second_rng);
        assert_eq!(second_controller.cave_collision_cooldown, 2);
        assert_eq!(
            second_controller.source_dword_10,
            i32::from(second_rng % 0x30) + 15
        );
        assert_eq!(
            second_controller.source_byte_3e,
            first_controller.source_byte_3e.wrapping_add(1)
        );
        assert!(
            controller
                .economy()
                .map_chains()
                .is_linked(stage_controller_slot)
        );
        assert!(controller.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Castle { source_slot, .. }
                    if *source_slot == castle_slot
            )
        }));

        // The even-phase maintenance call is exercised only by the ordinary
        // source route; no direct maintenance helper call is used here.
        let mut maintenance = inspection
            .level2_reset_session()
            .expect("cache-loaded maintenance session");
        maintenance.select_level2_campaign_entry_stage();
        let maintenance_slot = maintenance
            .economy_mut()
            .build_castle_with_footprint(Position::default(), 111, &stage_one)
            .expect("maintenance Castle allocation");
        maintenance.step_outer_turn(PlayerCommand::default(), 0);
        assert!(maintenance.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::DynamicArena {
                    kind: Level2DynamicArenaKind::CastleChild,
                    ..
                }
            )
        }));
        assert_eq!(
            (
                maintenance.economy().castle().expect("Castle").action,
                maintenance.economy().castle().expect("Castle").build_state,
            ),
            (5, 4)
        );
        {
            let castle = maintenance.economy_mut().castle.as_mut().expect("Castle");
            castle.action = 4;
            castle.timer = 0;
            castle.phase &= !1;
        }
        maintenance.step_outer_turn(PlayerCommand::default(), 0);
        let maintenance_defender_slot = maintenance
            .economy()
            .castle()
            .expect("Castle")
            .defender_slots
            .iter()
            .take(usize::from(
                maintenance
                    .economy()
                    .castle()
                    .expect("Castle")
                    .defender_count,
            ))
            .copied()
            .find(|slot| *slot != 0)
            .expect("source defender relation");
        let defender = maintenance
            .economy()
            .castle_children()
            .iter()
            .find(|child| {
                child.active
                    && child.source_entity_index == maintenance_defender_slot
                    && child.model == 3
                    && child.action == 9
            })
            .expect("even-phase Castle defender maintenance");
        assert!(
            maintenance
                .economy()
                .map_chains()
                .is_linked(defender.source_entity_index)
        );
        assert!(maintenance.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Castle { source_slot, .. }
                    if *source_slot == maintenance_slot
            )
        }));

        // Destruction indexes the loaded old-stage row, and reset restores
        // the cache-loaded arena, terrain authority, and campaign digest.
        let mut teardown = inspection
            .level2_reset_session()
            .expect("cache-loaded teardown session");
        teardown.select_level2_campaign_entry_stage();
        let baseline_campaign_hash = teardown.campaign_state_hash();
        let baseline_terrain_hash = teardown.terrain_hash();
        let baseline_free_slots = teardown.economy().free_slots.len();
        let teardown_slot = teardown
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("teardown Castle allocation");
        {
            let castle = teardown.economy_mut().castle.as_mut().expect("Castle");
            castle.action = 6;
            castle.level = 1;
            castle.life = -1;
        }
        teardown.step_outer_turn(PlayerCommand::default(), 0);
        assert!(teardown.economy().events().iter().any(|event| {
            matches!(
                event,
                Level2EconomyEvent::CastleStageDestroyed {
                    source_entity_index,
                    previous_level: 1,
                    next_level: 0,
                } if *source_entity_index == teardown_slot
            )
        }));
        assert!(!teardown.economy().map_chains().is_linked(teardown_slot));
        assert_ne!(teardown.terrain_hash(), baseline_terrain_hash);
        teardown.reset();
        assert_eq!(teardown.campaign_state_hash(), baseline_campaign_hash);
        assert_eq!(teardown.terrain_hash(), baseline_terrain_hash);
        assert_eq!(teardown.economy().free_slots.len(), baseline_free_slots);
        assert_eq!(teardown.economy().castle_stage_footprints.len(), 7);

        // A binding mutation with its stale payload digest is rejected before
        // any gameplay rows can be installed.
        let tampered_root = std::env::temp_dir().join(format!(
            "mc-bvihb1-gameplay-manifest-tamper-{}",
            std::process::id()
        ));
        if tampered_root.exists() {
            fs::remove_dir_all(&tampered_root).expect("remove stale tamper cache");
        }
        fs::create_dir_all(&tampered_root).expect("create tamper cache");
        fs::copy(
            gameplay_cache.join("manifest.json"),
            tampered_root.join("manifest.json"),
        )
        .expect("copy gameplay manifest");
        let manifest_path = tampered_root.join("manifest.json");
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).expect("read gameplay manifest"))
                .expect("parse gameplay manifest");
        manifest["bindings"]["level_export_package_sha256"] = json!("0".repeat(64));
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).expect("encode tampered manifest"),
        )
        .expect("write tampered manifest");
        assert!(NativeWorld::inspect_level2_with_owner_cache(&package, &tampered_root).is_err());
        let mut unsigned_manifest = manifest.clone();
        unsigned_manifest
            .as_object_mut()
            .expect("tampered manifest object")
            .remove("manifest_payload_sha256");
        let canonical_manifest = serde_json::to_string_pretty(&unsigned_manifest)
            .expect("encode tampered manifest canonically")
            + "\n";
        manifest["manifest_payload_sha256"] = json!(sha256_hex(canonical_manifest.as_bytes()));
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).expect("encode binding-tampered manifest"),
        )
        .expect("write binding-tampered manifest");
        assert!(NativeWorld::inspect_level2_with_owner_cache(&package, &tampered_root).is_err());
        fs::remove_dir_all(tampered_root).expect("remove test tamper cache");

        // Refresh the DAT declaration, component content identity, and
        // canonical manifest payload after changing a consumed BUILD byte.
        // The immutable qualified BUILD-root pin must still reject it.
        let dat_tampered_root = std::env::temp_dir().join(format!(
            "mc-bvihb1-gameplay-dat-tamper-{}",
            std::process::id()
        ));
        if dat_tampered_root.exists() {
            fs::remove_dir_all(&dat_tampered_root).expect("remove stale DAT tamper cache");
        }
        fs::create_dir_all(dat_tampered_root.join("gameplay")).expect("create DAT tamper cache");
        fs::copy(
            gameplay_cache.join("manifest.json"),
            dat_tampered_root.join("manifest.json"),
        )
        .expect("copy DAT tamper manifest");
        fs::copy(
            gameplay_cache.join("gameplay/DATA_BUILD0-0.TAB.bin"),
            dat_tampered_root.join("gameplay/DATA_BUILD0-0.TAB.bin"),
        )
        .expect("copy DAT tamper TAB");
        let dat_path = dat_tampered_root.join("gameplay/DATA_BUILD0-0.DAT.bin");
        fs::copy(
            gameplay_cache.join("gameplay/DATA_BUILD0-0.DAT.bin"),
            &dat_path,
        )
        .expect("copy DAT tamper DAT");
        let mut dat = fs::read(&dat_path).expect("read DAT tamper DAT");
        assert_eq!(dat.first(), Some(&255));
        dat[0] = 254;
        fs::write(&dat_path, &dat).expect("write DAT tamper DAT");
        let dat_sha256 = sha256_hex(&dat);
        let dat_manifest_path = dat_tampered_root.join("manifest.json");
        let mut dat_manifest: Value = serde_json::from_slice(
            &fs::read(&dat_manifest_path).expect("read DAT tamper manifest"),
        )
        .expect("parse DAT tamper manifest");
        let outputs = dat_manifest["outputs"]
            .as_array_mut()
            .expect("DAT tamper manifest outputs");
        let mut replaced_dat_declaration = false;
        for output in outputs {
            if output["path"] == json!("gameplay/DATA_BUILD0-0.DAT.bin") {
                output["sha256"] = json!(dat_sha256);
                replaced_dat_declaration = true;
            }
        }
        assert!(replaced_dat_declaration);
        let mut content_identity = String::new();
        {
            use std::fmt::Write as _;
            for output in dat_manifest["outputs"]
                .as_array()
                .expect("DAT tamper manifest outputs")
            {
                writeln!(
                    &mut content_identity,
                    "{}\0{}\0{}",
                    output["path"].as_str().expect("DAT tamper output path"),
                    output["sha256"].as_str().expect("DAT tamper output hash"),
                    output["bytes"].as_u64().expect("DAT tamper output bytes")
                )
                .expect("write DAT tamper content identity");
            }
        }
        dat_manifest["cache_content_sha256"] = json!(sha256_hex(content_identity.as_bytes()));
        let mut unsigned_dat_manifest = dat_manifest.clone();
        unsigned_dat_manifest
            .as_object_mut()
            .expect("DAT tamper manifest object")
            .remove("manifest_payload_sha256");
        let canonical_dat_manifest = serde_json::to_string_pretty(&unsigned_dat_manifest)
            .expect("encode DAT tamper manifest canonically")
            + "\n";
        dat_manifest["manifest_payload_sha256"] =
            json!(sha256_hex(canonical_dat_manifest.as_bytes()));
        fs::write(
            &dat_manifest_path,
            serde_json::to_vec_pretty(&dat_manifest).expect("encode DAT tamper manifest"),
        )
        .expect("write DAT tamper manifest");
        assert!(
            NativeWorld::inspect_level2_with_owner_cache(&package, &dat_tampered_root).is_err()
        );
        fs::remove_dir_all(dat_tampered_root).expect("remove DAT tamper cache");
    }

    #[test]
    fn level_two_economy_dispatch_uses_one_terrain_and_reset_authority() {
        let package = level_two_fixture_path();
        let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
            .expect("qualified Level 2 gameplay component");
        assert_eq!(inspection.level2_house_build_footprints.len(), 9);
        for building_index in [26_u8, 29, 30, 44, 50, 55, 59, 60, 64] {
            let footprint = inspection
                .level2_house_build_footprints
                .get(&building_index)
                .expect("qualified Level 2 House BUILD footprint");
            assert_eq!(footprint.index, building_index);
            assert_eq!(
                footprint.cells().len(),
                usize::from(footprint.width) * usize::from(footprint.height)
            );
        }

        let mut session = inspection
            .level2_reset_session()
            .expect("cache-loaded Level 2 session");
        session.select_level2_campaign_entry_stage();
        let baseline_terrain_hash = session.terrain_hash();
        let baseline_campaign_hash = session.campaign_state_hash();
        let baseline_ordinary_hash = session.ordinary().reset_hash();
        let baseline_free_count = session.economy().free_slots.len();
        let baseline_arena_slots = session.economy().live_arena_slots();
        let baseline_family_counts = (
            session
                .ordinary()
                .houses()
                .iter()
                .filter(|house| house.active)
                .count(),
            session
                .ordinary()
                .villagers()
                .iter()
                .filter(|villager| villager.active)
                .count(),
            session
                .ordinary()
                .dolmens()
                .iter()
                .filter(|dolmen| dolmen.entity().active)
                .count(),
            session.economy().live_arena_slots(),
        );
        let house = session
            .ordinary()
            .houses()
            .iter()
            .copied()
            .find(|house| house.parameter.produces_villagers())
            .expect("qualified Level 2 resident-producing House");
        assert!(
            inspection
                .level2_house_build_footprints
                .contains_key(&house.building_index)
        );

        // Possession is a source mailbox consumed by the same ordinary House
        // visit.  Its owner relation is projected into the shared economy
        // source-owner map, not a parallel test carrier.
        let possession_owner = session.state.entity_index;
        session
            .ordinary_mut()
            .submit_house_possession(house.entity_index, possession_owner, 1)
            .expect("qualified House possession mailbox");
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::House { events, .. }
                    if events.iter().any(|event| matches!(
                        event,
                        HouseStepEvent::Possessed {
                            entity_index,
                            owner_entity_index,
                            payload: 1,
                        } if *entity_index == house.entity_index
                            && *owner_entity_index == possession_owner
                    ))
            )
        }));
        assert_eq!(
            session.economy().source_owner(house.entity_index),
            possession_owner
        );
        session.reset();
        assert_eq!(session.economy().source_owner(house.entity_index), 0);

        // The qualified owner row starts with two occupants. The existing
        // resident-entry operation supplies the third occupant before the
        // ordinary House source visit; no House row is rewritten here.
        assert_eq!(house.occupants, 2);
        assert_eq!(
            session
                .ordinary_mut()
                .houses
                .try_enter_resident(house.entity_index),
            HouseResidentEntry::Entered
        );
        session
            .ordinary_mut()
            .inject_house_damage(house.entity_index, 1, 111)
            .expect("qualified House resident damage mailbox");
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::House { events, .. }
                    if events.iter().any(|event| matches!(
                        event,
                        HouseStepEvent::ResidentRequested {
                            entity_index,
                            model: 4,
                            action: 33,
                            ..
                        } if *entity_index == house.entity_index
                    ))
            )
        }));
        let resident = session
            .economy()
            .house_residents()
            .iter()
            .find(|resident| resident.parent_house_entity_index == house.entity_index)
            .copied()
            .expect("resident constructor reaches shared Level 2 arena");
        assert_eq!((resident.model, resident.action), (4, 33));
        assert_eq!(
            session.economy().source_owner(resident.source_entity_index),
            resident.owner_entity_index
        );
        assert!(
            session
                .economy()
                .map_chains()
                .is_linked(resident.source_entity_index)
        );
        session.reset();

        // Exhaustion rejects action-53's multi-resident transaction without
        // committing House state, arena allocation, RNG, or map surgery.
        session
            .ordinary_mut()
            .inject_house_damage(house.entity_index, house.life.wrapping_add(1), 111)
            .expect("exhaustion House damage mailbox");
        session.step_outer_turn(PlayerCommand::default(), 0);
        let failed_house = session
            .ordinary()
            .houses()
            .iter()
            .copied()
            .find(|candidate| candidate.entity_index == house.entity_index)
            .expect("failed House source row");
        session.economy_mut().free_slots.clear();
        let failed_terrain_hash = session.terrain_hash();
        let failed_map = session.economy().map_chains().linked_entity_indices();
        let failed_source_rng = session.economy().source_rng.clone();
        let failed_global_rng = session.economy().global_rng;
        let failed_live_arena = session.economy().live_arena_slots();
        let failed_house_owner = session.economy().source_owner(house.entity_index);
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
            event,
            Level2DispatchEvent::Error { source_slot, .. }
                if *source_slot == house.entity_index
            )
        }));
        assert!(!session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::House { source_slot, .. }
                    if *source_slot == house.entity_index
            )
        }));
        assert_eq!(
            session
                .ordinary()
                .houses()
                .iter()
                .find(|candidate| candidate.entity_index == house.entity_index)
                .copied(),
            Some(failed_house)
        );
        assert_eq!(session.economy().house_residents().len(), 0);
        assert_eq!(session.terrain_hash(), failed_terrain_hash);
        assert_eq!(session.economy().source_rng, failed_source_rng);
        assert_eq!(
            session.economy().global_rng,
            level2_next_rng32(failed_global_rng)
        );
        assert_eq!(session.economy().live_arena_slots(), failed_live_arena);
        assert_eq!(
            session.economy().source_owner(house.entity_index),
            failed_house_owner
        );
        assert_eq!(
            session.economy().map_chains().linked_entity_indices(),
            failed_map
        );
        session.reset();
        assert_eq!(session.terrain_hash(), baseline_terrain_hash);
        assert_eq!(session.campaign_state_hash(), baseline_campaign_hash);

        // Action-53 returns source-ordered resident constructors as part of
        // the same ordinary removal transaction.
        session
            .ordinary_mut()
            .inject_house_damage(house.entity_index, house.life.wrapping_add(1), 111)
            .expect("qualified House damage mailbox");
        session.step_outer_turn(PlayerCommand::default(), 0);
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::House { events, .. }
                    if events.iter().any(|event| matches!(
                        event,
                        HouseStepEvent::RemoveFootprint { entity_index, .. }
                            if *entity_index == house.entity_index
                    ))
            )
        }));
        assert_eq!(session.terrain, session.ordinary().terrain);
        assert_eq!(session.terrain, session.economy().terrain);
        assert_ne!(session.terrain_hash(), baseline_terrain_hash);
        let removal_residents = session
            .economy()
            .house_residents()
            .iter()
            .filter(|resident| resident.parent_house_entity_index == house.entity_index)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(
            removal_residents
                .iter()
                .map(|resident| (resident.model, resident.action))
                .collect::<Vec<_>>(),
            vec![(4, 33), (12, 97)]
        );
        let mut expected_resident_slots = removal_residents
            .iter()
            .map(|resident| resident.source_entity_index)
            .collect::<Vec<_>>();
        expected_resident_slots.sort_unstable();
        let dispatched_resident_slots = session
            .dispatch_events()
            .iter()
            .filter_map(|event| match event {
                Level2DispatchEvent::DynamicArena {
                    source_slot,
                    kind: Level2DynamicArenaKind::HouseResident,
                    ..
                } => Some(*source_slot),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(dispatched_resident_slots, expected_resident_slots);
        for resident in &removal_residents {
            assert!(
                session
                    .economy()
                    .map_chains()
                    .is_linked(resident.source_entity_index)
            );
        }
        let replay_dispatch = session.dispatch_events().to_vec();
        let replay_terrain_hash = session.terrain_hash();
        let replay_campaign_hash = session.campaign_state_hash();
        let replay_ordinary_hash = session.ordinary().reset_hash();

        session.reset();
        assert_eq!(session.terrain_hash(), baseline_terrain_hash);
        assert_eq!(session.campaign_state_hash(), baseline_campaign_hash);
        assert_eq!(session.ordinary().reset_hash(), baseline_ordinary_hash);
        assert_eq!(session.economy().free_slots.len(), baseline_free_count);
        assert_eq!(session.economy().live_arena_slots(), baseline_arena_slots);
        assert!(session.economy().house_residents().is_empty());
        assert_eq!(
            (
                session
                    .ordinary()
                    .houses()
                    .iter()
                    .filter(|house| house.active)
                    .count(),
                session
                    .ordinary()
                    .villagers()
                    .iter()
                    .filter(|villager| villager.active)
                    .count(),
                session
                    .ordinary()
                    .dolmens()
                    .iter()
                    .filter(|dolmen| dolmen.entity().active)
                    .count(),
                session.economy().live_arena_slots(),
            ),
            baseline_family_counts
        );

        session
            .ordinary_mut()
            .inject_house_damage(house.entity_index, house.life.wrapping_add(1), 111)
            .expect("replay House damage mailbox");
        session.step_outer_turn(PlayerCommand::default(), 0);
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(session.dispatch_events(), replay_dispatch.as_slice());
        assert_eq!(session.terrain_hash(), replay_terrain_hash);
        assert_eq!(session.campaign_state_hash(), replay_campaign_hash);
        assert_eq!(session.ordinary().reset_hash(), replay_ordinary_hash);
    }

    #[test]
    fn level_two_castle_root_positions_follow_source_grid_and_anchor_vector() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let terrain = SessionTerrain::from_inspection(&inspection);
        let footprint = CastleBuildFootprint::from_owner_cells(1, 8, 8, &[0xff; 128])
            .expect("source 8x8 BUILD vector");
        let projectile = Position {
            x: 0x13_45,
            y: 0x56_78,
            z: 1_234,
        };
        // Make the selector error observable: every candidate 4x4 and 8x8
        // perimeter cell is high, except `(16,82)`, which belongs only to
        // the consumer-effective 8x8 boundary. A second VGA-halving pass
        // therefore reports 200*32 instead of the source 0*32 anchor.
        let mut boundary_terrain = terrain.clone();
        for (width, height) in [(8_u8, 8_u8), (4, 4)] {
            let center_x = 20_u8;
            let center_y = 86_u8;
            let start_x = center_x - width / 2;
            let start_y = center_y - height / 2;
            for offset in 0..width {
                boundary_terrain.planes.ground_heights[terrain_index(start_x + offset, start_y)] =
                    200;
                boundary_terrain.planes.ground_heights
                    [terrain_index(start_x + offset, start_y + height)] = 200;
            }
            for offset in 0..height {
                boundary_terrain.planes.ground_heights[terrain_index(start_x, start_y + offset)] =
                    200;
                boundary_terrain.planes.ground_heights
                    [terrain_index(start_x + width, start_y + offset)] = 200;
            }
        }
        boundary_terrain.planes.ground_heights[terrain_index(16, 82)] = 0;
        let (boundary_root, boundary_anchor) =
            level2_castle_root_positions(&boundary_terrain, &footprint, projectile);
        assert_eq!((boundary_root.x, boundary_root.y), (0x14_00, 0x56_00));
        assert_eq!(boundary_anchor.z, 0);
        let (_, halved_anchor) = level2_castle_root_positions_with_resolution(
            &boundary_terrain,
            &footprint,
            projectile,
            true,
        );
        assert_eq!(halved_anchor.z, 200 * 32);
        let (root, anchor) = level2_castle_root_positions(&terrain, &footprint, projectile);
        assert_eq!((root.x, root.y), (0x14_00, 0x56_00));
        assert_eq!((anchor.x, anchor.y), (root.x, root.y));
        let center_x = (root.x >> 8) as u8;
        let center_y = (root.y >> 8) as u8;
        let width = footprint.width;
        let height = footprint.height;
        let start_x = center_x.wrapping_sub(width / 2);
        let start_y = center_y.wrapping_sub(height / 2);
        let mut minimum = 250_u8;
        for offset in 0..width {
            for (x, y) in [
                (start_x.wrapping_add(offset), start_y),
                (start_x.wrapping_add(offset), start_y.wrapping_add(height)),
            ] {
                minimum = minimum.min(
                    (terrain.ground_height(Position {
                        x: u16::from(x) << 8,
                        y: u16::from(y) << 8,
                        z: 0,
                    }) >> 5) as u8,
                );
            }
        }
        for offset in 0..height {
            for (x, y) in [
                (start_x.wrapping_add(width), start_y.wrapping_add(offset)),
                (start_x, start_y.wrapping_add(offset)),
            ] {
                minimum = minimum.min(
                    (terrain.ground_height(Position {
                        x: u16::from(x) << 8,
                        y: u16::from(y) << 8,
                        z: 0,
                    }) >> 5) as u8,
                );
            }
        }
        assert_eq!(anchor.z, i16::from(minimum) * 32);

        let mut session = inspection
            .level2_reset_session()
            .expect("Level 2 placement session");
        let castle_slot = session
            .economy_mut()
            .build_castle_with_footprint(projectile, 111, &footprint)
            .expect("source-derived Castle placement");
        let castle = session.economy().castle().expect("Castle");
        assert_eq!(castle.source_entity_index, castle_slot);
        assert_eq!(castle.position, root);
        assert_eq!(castle.anchor_position, anchor);
        assert_eq!(
            (
                castle.action,
                castle.build_state,
                castle.level,
                castle.life,
                castle.max_life,
                castle.mana_capacity
            ),
            (5, 0, 0, 40_000, 40_000, 0)
        );
        assert_eq!(session.economy().player_castle_entity_index(), 0);
        assert!(session.economy().map_chains().is_linked(castle_slot));
        assert!(session.economy_mut().step_castle_action(castle_slot));
        let stage_controller = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.active && child.model == 79)
            .expect("stage-one model-79 controller");
        assert_eq!(
            (stage_controller.position.x, stage_controller.position.y),
            (root.x, root.y)
        );
        assert_eq!(stage_controller.chain_previous_slot, castle_slot);
    }

    #[test]
    fn level_two_castle_controller_points_pin_stage_corner_vectors() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let vectors: &[(u8, &[(i8, i8)])] = &[
            (1, &[(0, 0)]),
            (2, &[(-7, -7), (7, -7), (-7, 7), (7, 7)]),
            (4, &[(-14, -14), (14, -14), (-14, 14), (14, 14)]),
            (
                6,
                &[
                    (-21, -21),
                    (0, -21),
                    (21, -21),
                    (-21, 0),
                    (21, 0),
                    (-21, 21),
                    (0, 21),
                    (21, 21),
                ],
            ),
        ];
        for &(level, expected_offsets) in vectors {
            let mut session = inspection
                .level2_reset_session()
                .expect("controller vector session");
            let root_position = Position {
                x: 0x20_00,
                y: 0x30_00,
                z: 0,
            };
            let castle_slot = session
                .economy_mut()
                .build_castle(root_position, 111)
                .expect("controller vector Castle");
            {
                let castle = session
                    .economy_mut()
                    .castle
                    .as_mut()
                    .expect("controller vector Castle");
                castle.action = 5;
                castle.build_state = 3;
                castle.level = level.saturating_sub(1);
            }
            assert!(session.economy_mut().step_castle_action(castle_slot));
            let castle = session
                .economy()
                .castle()
                .expect("controller vector Castle");
            assert_eq!(
                castle.controller_chain_count as usize,
                expected_offsets.len()
            );
            let slots = &castle.controller_chain_slots[..expected_offsets.len()];
            for (ordinal, (&slot, &(offset_x, offset_y))) in
                slots.iter().zip(expected_offsets).enumerate()
            {
                let child = session
                    .economy()
                    .castle_children()
                    .iter()
                    .find(|child| child.source_entity_index == slot)
                    .expect("controller child slot");
                let expected_x = root_position
                    .x
                    .wrapping_add_signed(i16::from(offset_x) * 256);
                let expected_y = root_position
                    .y
                    .wrapping_add_signed(i16::from(offset_y) * 256);
                assert_eq!(
                    (child.position.x, child.position.y),
                    (expected_x, expected_y)
                );
                assert_eq!(
                    child.chain_previous_slot,
                    if ordinal == 0 {
                        castle_slot
                    } else {
                        slots[ordinal - 1]
                    }
                );
                assert_eq!(
                    child.chain_next_slot,
                    slots.get(ordinal + 1).copied().unwrap_or_default()
                );
            }
        }
    }

    #[test]
    fn level_two_castle_controller_coefficients_back_down_to_nonzero_stage() {
        assert_eq!(level2_castle_shift_axes(1), (1_664, 1_664));
        assert_eq!(level2_castle_shift_axes(2), (3_328, 3_328));
        assert_eq!(level2_castle_shift_axes(4), (5_120, 5_120));
        assert_eq!(level2_castle_shift_axes(6), (6_784, 6_784));
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("controller fallback session");
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("controller fallback Castle");
        {
            let economy = session.economy_mut();
            assert!(economy.set_castle_controller_coefficient(2, 0x2a));
            assert!(economy.set_castle_controller_coefficient(3, 0));
            let castle = economy.castle.as_mut().expect("controller fallback Castle");
            castle.action = 5;
            castle.build_state = 3;
            castle.level = 2;
        }
        assert!(session.economy_mut().step_castle_action(castle_slot));
        let castle = session
            .economy()
            .castle()
            .expect("controller fallback Castle");
        assert_eq!(castle.level, 3);
        assert_eq!(castle.controller_chain_count, 4);
        for &slot in &castle.controller_chain_slots[..4] {
            let child = session
                .economy()
                .castle_children()
                .iter()
                .find(|child| child.source_entity_index == slot)
                .expect("fallback model-79 child");
            assert_eq!((child.source_byte_43, child.source_word_4a), (0x2a, 2));
        }
    }

    #[test]
    fn level_two_castle_controller_selection_backs_down_zero_coefficients() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("controller coefficient session");
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("controller coefficient Castle");
        {
            let economy = session.economy_mut();
            assert!(economy.set_castle_controller_coefficient(1, 0x2a));
            assert!(economy.set_castle_controller_coefficient(2, 0));
            economy.free_slots.truncate(1);
            let castle = economy
                .castle
                .as_mut()
                .expect("controller coefficient Castle");
            castle.action = 6;
            castle.level = 2;
        }
        assert!(session.economy_mut().step_castle_action(castle_slot));
        let castle = session.economy().castle().expect("downgraded Castle");
        assert_eq!(castle.controller_chain_count, 1);
        let child = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.active && child.model == 79)
            .expect("backed-down controller");
        assert_eq!((child.source_byte_43, child.source_word_4a), (0x2a, 1));

        let mut no_controller = inspection
            .level2_reset_session()
            .expect("zero coefficient session");
        let zero_slot = no_controller
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("zero coefficient Castle");
        {
            let economy = no_controller.economy_mut();
            for stage in 1..=7 {
                assert!(economy.set_castle_controller_coefficient(stage, 0));
            }
            let castle = economy.castle.as_mut().expect("zero coefficient Castle");
            castle.action = 6;
            castle.level = 2;
        }
        assert!(no_controller.economy_mut().step_castle_action(zero_slot));
        assert_eq!(
            no_controller
                .economy()
                .castle()
                .expect("zero coefficient Castle")
                .controller_chain_count,
            0
        );
    }

    #[test]
    fn level_two_castle_stage_removal_uses_old_tab_and_sentinel_rng() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("stage removal session");
        let mut stage_two_payload = vec![0xff; 21 * 21 * 2];
        stage_two_payload[..2].copy_from_slice(&[0, 21]);
        let footprint = CastleBuildFootprint::from_owner_cells(2, 21, 21, &stage_two_payload)
            .expect("stage-two BUILD record");
        assert!(
            session
                .economy_mut()
                .set_castle_stage_footprint(2, footprint)
        );
        session.reset();
        let baseline_hash = session.economy().reset_hash();
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("stage removal Castle");
        let source_seed = session
            .economy()
            .source_rng
            .get(&castle_slot)
            .copied()
            .expect("live Castle source RNG");
        let sentinel_seed = session.economy().sentinel_rng;
        {
            session.terrain.planes.ground_heights[terrain_index(246, 246)] = 100;
            let economy = session.economy_mut();
            let castle = economy.castle.as_mut().expect("stage removal Castle");
            castle.action = 6;
            castle.level = 2;
        }
        session.select_level2_campaign_entry_stage();
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Castle { source_slot, .. }
                    if *source_slot == castle_slot
            )
        }));
        let source_after = session
            .economy()
            .source_rng
            .get(&castle_slot)
            .copied()
            .expect("updated Castle source RNG");
        assert_eq!(source_after, source_seed);
        let first_sentinel_draw = level2_next_rng16(sentinel_seed);
        let expected_sentinel = if first_sentinel_draw % 50 <= 20 {
            first_sentinel_draw
        } else {
            level2_next_rng16(first_sentinel_draw)
        };
        let sentinel_after = session.economy().sentinel_rng;
        assert_eq!(sentinel_after, expected_sentinel);
        assert_ne!(sentinel_after, sentinel_seed);
        let changed_hash = session.economy().reset_hash();
        let mut sentinel_unchanged = session.economy().clone();
        sentinel_unchanged.sentinel_rng = sentinel_seed;
        assert_ne!(
            changed_hash,
            sentinel_unchanged.reset_hash(),
            "stage-removal sentinel RNG is hash-bound"
        );
        assert_ne!(changed_hash, baseline_hash);
        assert_eq!(
            session.economy().castle().expect("downgraded Castle").level,
            1
        );
        session.reset();
        assert_eq!(session.economy().reset_hash(), baseline_hash);
        assert_eq!(session.economy().sentinel_rng, sentinel_seed);
        assert!(!session.economy().source_rng.contains_key(&castle_slot));
    }

    #[test]
    fn level_two_castle_model42_handler_preserves_clock_before_root_consumer() {
        let package = level_two_fixture_path();
        let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
            .expect("qualified Level 2 gameplay component");
        let footprint = inspection
            .level2_castle_stage_footprints
            .get(&1)
            .expect("stage-one BUILD footprint")
            .clone();
        let mut session = inspection
            .level2_reset_session()
            .expect("model-42 BUILD handler session");
        let baseline_hash = session.economy().reset_hash();
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("model-42 BUILD root");
        let anchor = Position {
            x: 3_072,
            y: 4_096,
            z: 0,
        };
        assert!(session.economy_mut().set_castle_anchor(castle_slot, anchor));
        assert!(session.economy_mut().start_castle_construction(castle_slot));
        let expected_terrain = {
            let economy = session.economy();
            let mut expected = economy.terrain.clone();
            expected
                .apply_castle_update(
                    CastleTerrainUpdate::ConstructionPass {
                        position: anchor,
                        remaining: 18,
                    },
                    &footprint,
                )
                .expect("stage-one BUILD pass");
            expected
        };
        assert!(session.economy_mut().step_castle_action(castle_slot));
        let child_slot = session
            .economy()
            .castle()
            .expect("BUILD root")
            .construction_slot;
        assert!(
            session
                .economy_mut()
                .step_castle_child_terminal_slot(child_slot)
        );
        let initialized = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == child_slot)
            .copied()
            .expect("first model-42 BUILD callback");
        assert_eq!(initialized.source_word_2e, 0);
        assert_eq!(initialized.construction_target_height, 0);
        assert_eq!(initialized.source_dword_10, 18);
        assert_eq!(
            initialized.source_byte_3e,
            (child_slot as u8).wrapping_add(1),
            "BUILD callback epilogue advances byte_0x3E exactly once"
        );
        assert_eq!(initialized.source_byte_3b, 1);
        assert_eq!(
            initialized.control_flags & LEVEL2_CASTLE_CONSTRUCTION_INITIALIZED_BIT,
            LEVEL2_CASTLE_CONSTRUCTION_INITIALIZED_BIT
        );
        assert_ne!(initialized.control_flags & 1, 0);
        assert_eq!(
            session.economy().terrain.planes,
            expected_terrain.planes,
            "37BC0 applies the canonical BUILD payload on its first callback"
        );

        for _ in 0..17 {
            assert!(
                session
                    .economy_mut()
                    .step_castle_child_terminal_slot(child_slot)
            );
        }
        assert_eq!(
            session
                .economy()
                .castle_children()
                .iter()
                .find(|child| child.source_entity_index == child_slot)
                .expect("positive BUILD clock")
                .source_dword_10,
            1
        );
        assert!(
            session
                .economy_mut()
                .step_castle_child_terminal_slot(child_slot)
        );
        let timer_one = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == child_slot)
            .copied()
            .expect("BUILD timer-one transition");
        assert_eq!(timer_one.source_dword_10, -25);

        for _ in 0..24 {
            assert!(
                session
                    .economy_mut()
                    .step_castle_child_terminal_slot(child_slot)
            );
        }
        assert_eq!(
            session
                .economy()
                .castle_children()
                .iter()
                .find(|child| child.source_entity_index == child_slot)
                .expect("negative BUILD clock")
                .source_dword_10,
            -1
        );
        assert!(
            session
                .economy_mut()
                .step_castle_child_terminal_slot(child_slot)
        );
        assert_eq!(
            session
                .economy()
                .castle_children()
                .iter()
                .find(|child| child.source_entity_index == child_slot)
                .expect("completed BUILD clock")
                .source_dword_10,
            0
        );
        let state_two = session.economy().castle().expect("completed BUILD root");
        assert_eq!((state_two.action, state_two.build_state), (5, 2));
        assert_eq!(state_two.source_word_2e, 2);
        assert!(!state_two.construction_child_active);
        let hidden_child = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == child_slot)
            .copied()
            .expect("hidden model-42 BUILD child remains allocated");
        let hidden_generation = session.economy().slot_generation(child_slot);
        assert!(hidden_child.active);
        assert_ne!(hidden_child.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
        assert!(!session.economy().map_chains().is_linked(child_slot));

        // `UpdateEntities_57730` owns this reclaim, not the Castle root. A
        // destroyed root must not strand its already-hidden BUILD child;
        // after the generic prepass the old identity is gone, and the lowest
        // freed slot may be reused by the same ordinary turn.
        let mut root_destroyed = session.clone();
        let root_destroyed_generation = root_destroyed.economy().slot_generation(child_slot);
        root_destroyed
            .economy_mut()
            .castle
            .as_mut()
            .expect("root-gone Castle")
            .active = false;
        root_destroyed.select_level2_campaign_entry_stage();
        root_destroyed.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            root_destroyed
                .economy()
                .castle_children()
                .iter()
                .all(|child| {
                    !(child.active
                        && child.source_entity_index == child_slot
                        && child.source_class == hidden_child.source_class
                        && child.model == hidden_child.model
                        && child.action == hidden_child.action
                        && child.id_entity_index == hidden_child.id_entity_index
                        && child.owner_entity_index == hidden_child.owner_entity_index
                        && child.parent_entity_index == hidden_child.parent_entity_index)
                })
        );
        match root_destroyed
            .economy()
            .live_arena_cursor_rows()
            .into_iter()
            .find(|row| row.source_slot == child_slot)
        {
            None => {
                assert!(!root_destroyed.economy().map_chains().is_linked(child_slot));
                assert!(root_destroyed.economy().free_slots.contains(&child_slot));
            }
            Some(row) => {
                assert_ne!(row.generation, root_destroyed_generation);
                assert_eq!(row.kind, Level2DynamicArenaKind::AuthoredBee);
                assert!(!root_destroyed.economy().free_slots.contains(&child_slot));
                assert!(root_destroyed.economy().map_chains().is_linked(child_slot));
            }
        }

        session.select_level2_campaign_entry_stage();
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            session.economy().castle_children().iter().all(|child| {
                !(child.active
                    && child.source_entity_index == child_slot
                    && child.source_class == hidden_child.source_class
                    && child.model == hidden_child.model
                    && child.action == hidden_child.action
                    && child.id_entity_index == hidden_child.id_entity_index
                    && child.owner_entity_index == hidden_child.owner_entity_index
                    && child.parent_entity_index == hidden_child.parent_entity_index)
            }),
            "following UpdateEntities prologue reclaims the old model-42 BUILD row"
        );
        let slot_owner = session
            .economy()
            .live_arena_cursor_rows()
            .into_iter()
            .find(|row| row.source_slot == child_slot);
        match slot_owner {
            None => {
                assert!(!session.economy().map_chains().is_linked(child_slot));
                assert!(session.economy().free_slots.contains(&child_slot));
            }
            Some(row) => {
                // The source cursor may reuse the reclaimed lowest slot in
                // the same ordinary turn. Keep this assertion at the cursor
                // layer: the exact owner is the new source generation, not
                // necessarily another Castle-child struct.
                assert_ne!(row.generation, hidden_generation);
                assert_eq!(row.kind, Level2DynamicArenaKind::AuthoredBee);
                assert!(!session.economy().free_slots.contains(&child_slot));
                assert!(session.economy().map_chains().is_linked(child_slot));
            }
        }
        let consumed = session.economy().castle().expect("consumed root");
        assert_eq!((consumed.action, consumed.build_state), (4, 0));
        assert_ne!(session.economy().reset_hash(), baseline_hash);
        session.reset();
        assert_eq!(session.economy().reset_hash(), baseline_hash);
    }

    #[test]
    fn level_two_castle_outer_turn_sub5fd00_uses_source_share_and_one_draw_rng() {
        let package = level_two_fixture_path();
        let inspection = NativeWorld::inspect_level2(&package).expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("qualified Level 2 reset session");
        let baseline_hash = session.economy().reset_hash();
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("sub_5FD00 Castle");
        let global_before = session.economy().global_rng;
        let source_rng_before = *session
            .economy()
            .source_rng
            .get(&castle_slot)
            .expect("Castle source RNG");
        let before_spheres = session.economy().spheres().len();
        {
            let castle = session.economy_mut().castle.as_mut().expect("Castle");
            castle.level = 1;
            castle.phase = 0;
            castle.mana = 7_500;
            castle.mana_capacity = 1_000;
        }

        // The ordinary Castle root route releases 6,500 excess mana. Source
        // sub_5FD00 uses six equal 1,083-unit shares and leaves the final two
        // excess units on the Castle; it does not top up the final sphere to
        // 1,085.
        session.select_level2_campaign_entry_stage();
        session.step_outer_turn(PlayerCommand::default(), 0);
        let new_spheres = &session.economy().spheres()[before_spheres..];
        assert_eq!(new_spheres.len(), 6);
        assert!(new_spheres.iter().all(|sphere| sphere.mana == 1_083));
        assert_eq!(session.economy().castle().expect("Castle").mana, 1_002);
        assert_eq!(
            session
                .economy()
                .events()
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        Level2EconomyEvent::ManaSphereSpawned {
                            source_entity_index,
                            mana: 1_083,
                            ..
                        } if *source_entity_index == castle_slot
                    )
                })
                .count(),
            6
        );

        // NewEvent uses the post-pass global stream for every allocated slot;
        // sub_5FD00 advances each new sphere RNG exactly once and advances the
        // Castle RNG twice per sphere for radius then yaw.
        let global_after = level2_next_rng32(global_before);
        for sphere in new_spheres {
            let seed = sphere.source_entity_index.wrapping_add(global_after as u16);
            let one_draw = level2_next_rng16(seed);
            let two_draws = level2_next_rng16(one_draw);
            assert_eq!(sphere.entity_rng, one_draw);
            assert_ne!(sphere.entity_rng, two_draws);
        }
        let mut expected_source_rng = source_rng_before;
        for _ in 0..12 {
            expected_source_rng = level2_next_rng16(expected_source_rng);
        }
        assert_eq!(
            session.economy().source_rng.get(&castle_slot),
            Some(&expected_source_rng)
        );
        assert_ne!(session.economy().reset_hash(), baseline_hash);
        session.reset();
        assert_eq!(session.economy().reset_hash(), baseline_hash);

        // Insufficient mana does not enter the overflow allocator even though
        // the same qualified ordinary root route is visited.
        let mut no_overflow = inspection
            .level2_reset_session()
            .expect("qualified Level 2 reset session");
        let no_overflow_slot = no_overflow
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("no-overflow Castle");
        let no_overflow_before = no_overflow.economy().spheres().len();
        {
            let castle = no_overflow
                .economy_mut()
                .castle
                .as_mut()
                .expect("no-overflow Castle");
            castle.level = 1;
            castle.phase = 0;
            castle.mana = 1_000;
            castle.mana_capacity = 1_000;
        }
        no_overflow.select_level2_campaign_entry_stage();
        no_overflow.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(
            no_overflow.economy().spheres().len(),
            no_overflow_before,
            "sub_5FD00 does not allocate without excess mana"
        );
        assert_eq!(
            no_overflow
                .economy()
                .castle()
                .expect("no-overflow Castle")
                .mana,
            1_000
        );
        assert!(no_overflow.dispatch_events().iter().any(|event| matches!(
            event,
            Level2DispatchEvent::Castle { source_slot, .. }
                if *source_slot == no_overflow_slot
        )));
    }

    #[test]
    fn level_two_castle_model42_stage_two_composes_prior_build_cells() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("stage-two composite session");

        let mut stage_one_payload = vec![0xff_u8; 8 * 8 * 2];
        stage_one_payload[(4 * 8 + 4) * 2..(4 * 8 + 4) * 2 + 2].copy_from_slice(&[7, 4]);
        let stage_one = CastleBuildFootprint::from_owner_cells(1, 8, 8, &stage_one_payload)
            .expect("synthetic stage-one BUILD");

        let mut stage_two_payload = vec![0xff_u8; 21 * 21 * 2];
        stage_two_payload[..2].copy_from_slice(&[9, 6]);
        let stage_two = CastleBuildFootprint::from_owner_cells(2, 21, 21, &stage_two_payload)
            .expect("synthetic stage-two BUILD");
        assert_eq!(stage_two.cells()[10 * 21 + 10], [0xff, 0xff]);
        let stage_three =
            CastleBuildFootprint::from_owner_cells(3, 21, 21, &vec![0xff_u8; 21 * 21 * 2])
                .expect("synthetic stage-three BUILD");
        let stage_four =
            CastleBuildFootprint::from_owner_cells(4, 35, 35, &vec![0xff_u8; 35 * 35 * 2])
                .expect("synthetic stage-four BUILD");
        let mut stage_five_payload = vec![0xff_u8; 35 * 35 * 2];
        stage_five_payload[..2].copy_from_slice(&[0x11, 0x0c]);
        let stage_five = CastleBuildFootprint::from_owner_cells(5, 35, 35, &stage_five_payload)
            .expect("synthetic odd stage-five BUILD");
        let stage_six_payload = vec![0xff_u8; 48 * 48 * 2];
        let stage_six = CastleBuildFootprint::from_owner_cells(6, 48, 48, &stage_six_payload)
            .expect("synthetic stage-six BUILD");
        assert!(
            session
                .economy_mut()
                .set_castle_stage_footprint(1, stage_one)
        );
        assert!(
            session
                .economy_mut()
                .set_castle_stage_footprint(2, stage_two)
        );
        assert!(
            session
                .economy_mut()
                .set_castle_stage_footprint(3, stage_three)
        );
        assert!(
            session
                .economy_mut()
                .set_castle_stage_footprint(4, stage_four)
        );
        assert!(
            session
                .economy_mut()
                .set_castle_stage_footprint(5, stage_five)
        );
        assert!(
            session
                .economy_mut()
                .set_castle_stage_footprint(6, stage_six)
        );
        session.reset();

        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("stage-two composite Castle");
        let anchor = Position {
            x: 3_072,
            y: 4_096,
            z: 0,
        };
        assert!(session.economy_mut().set_castle_anchor(castle_slot, anchor));
        {
            let castle = session
                .economy_mut()
                .castle
                .as_mut()
                .expect("stage-two Castle");
            castle.action = 5;
            castle.build_state = 0;
            castle.level = 1;
            castle.source_word_2e = 2;
        }
        assert!(session.economy_mut().step_castle_action(castle_slot));
        let child_slot = session
            .economy()
            .castle()
            .expect("stage-two Castle root")
            .construction_slot;

        let composite = session
            .economy()
            .composite_castle_construction_footprint(2)
            .expect("composite stage-two BUILD");
        assert_eq!(composite.cells()[10 * 21 + 10], [7, 4]);
        assert_eq!(composite.cells()[0], [9, 6]);

        let expected_footprint = CastleBuildFootprint::from_owner_cells(2, 21, 21, &{
            let mut payload = vec![0xff_u8; 21 * 21 * 2];
            payload[(10 * 21 + 10) * 2..(10 * 21 + 10) * 2 + 2].copy_from_slice(&[7, 4]);
            payload[..2].copy_from_slice(&[9, 6]);
            payload
        })
        .expect("expected composite stage-two BUILD");
        let mut expected_terrain = session.economy().terrain.clone();
        expected_terrain
            .apply_castle_update(
                CastleTerrainUpdate::ConstructionPass {
                    position: anchor,
                    remaining: 18,
                },
                &expected_footprint,
            )
            .expect("expected composite BUILD pass");

        assert!(
            session
                .economy_mut()
                .step_castle_child_terminal_slot(child_slot)
        );
        assert_eq!(
            session.economy().terrain.planes,
            expected_terrain.planes,
            "37BC0 must preserve BUILD1 data inside the stage-two ff hole"
        );
        let center_x = anchor.x.wrapping_add(128).to_be_bytes()[0];
        let center_y = anchor.y.wrapping_add(128).to_be_bytes()[0];
        let inner_index = terrain_index(center_x, center_y);
        let outer_index = terrain_index(center_x.wrapping_sub(10), center_y.wrapping_sub(10));
        assert_eq!(
            session.economy().terrain.planes.ground_heights[inner_index],
            expected_terrain.planes.ground_heights[inner_index]
        );
        assert_eq!(
            session.economy().terrain.planes.terrain_types[outer_index],
            expected_terrain.planes.terrain_types[outer_index]
        );

        let odd_composite = session
            .economy()
            .composite_castle_construction_footprint(6)
            .expect("composite odd stage-six BUILD");
        assert_eq!(
            odd_composite.cells()[7 * 48 + 7],
            [0x11, 0x0c],
            "source centers stage-five origin at 24>>1 - 35>>1 = 7"
        );
        assert_ne!(
            odd_composite.cells()[6 * 48 + 6],
            [0x11, 0x0c],
            "midpoint subtraction would incorrectly place the marker at 6"
        );
    }

    #[test]
    fn level_two_castle_material_11_uses_source_corner_table() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("material-11 Castle session");
        let position = Position {
            x: 3_072,
            y: 4_096,
            z: 0,
        };
        let x = position.x.wrapping_add(128).to_be_bytes()[0];
        let y = position.y.wrapping_add(128).to_be_bytes()[0];
        let index = terrain_index(x, y);
        let before = session.economy().terrain.clone();
        let corner = classify_castle_corner(&before.planes, x, y, 7);
        let table_offset = 0x40 + 2 * corner;
        let footprint = CastleBuildFootprint::from_owner_cells(3, 1, 1, &[0x11, 0xff])
            .expect("material-11 BUILD cell");

        assert!(
            session
                .economy_mut()
                .terrain
                .apply_castle_update(
                    CastleTerrainUpdate::ConstructionPass {
                        position,
                        remaining: 1,
                    },
                    &footprint,
                )
                .is_ok()
        );

        let after = &session.economy().terrain.planes;
        assert_eq!(
            after.terrain_types[index], TERRAIN_GENERATION_TABLE_PREFIX[table_offset],
            "material 0x11 must select the source 0x40 corner table"
        );
        assert_eq!(
            after.angles[index],
            ((before.planes.angles[index] & 0x8f)
                | TERRAIN_GENERATION_TABLE_PREFIX[table_offset + 1]
                | 0x80)
                & !0x08
        );
        for (corner_x, corner_y) in [
            (x, y),
            (x.wrapping_add(1), y),
            (x.wrapping_add(1), y.wrapping_add(1)),
            (x, y.wrapping_add(1)),
        ] {
            assert_eq!(
                after.angles[terrain_index(corner_x, corner_y)] & 0x08,
                0,
                "material 0x11 keeps the existing Day neighbor-angle handling"
            );
        }
    }

    #[test]
    fn level_two_castle_model41_msprd_is_separate_from_model42_build() {
        assert_eq!(
            LEVEL2_CASTLE_MODEL41_MSPRD_RECTS,
            [
                (0, 0),
                (16, 11),
                (16, 11),
                (16, 9),
                (16, 9),
                (16, 11),
                (1, 0),
                (1, 0)
            ]
        );
        assert_eq!(level2_castle_model41_msprd_rect(6), Some((1, 0)));

        let package = level_two_fixture_path();
        let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
            .expect("qualified Level 2 gameplay component");
        let mut session = inspection
            .level2_reset_session()
            .expect("model-41 discriminator session");
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("discriminator Castle");
        let anchor = Position {
            x: 3_072,
            y: 4_096,
            z: 0,
        };
        assert!(session.economy_mut().set_castle_anchor(castle_slot, anchor));
        assert!(session.economy_mut().start_castle_construction(castle_slot));
        assert!(session.economy_mut().step_castle_action(castle_slot));
        session.economy_mut().terrain.planes.ground_heights.fill(20);
        assert!(
            session
                .economy_mut()
                .start_castle_defense_child(castle_slot)
        );
        assert!(session.economy_mut().step_castle_action(castle_slot));
        let defense_slot = session
            .economy()
            .castle()
            .expect("defense root")
            .defense_slot;
        let before = session.economy().terrain.planes.clone();
        assert!(
            session
                .economy_mut()
                .step_castle_child_terminal_slot(defense_slot)
        );
        let defense = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defense_slot)
            .copied()
            .expect("model-41/MSPRD child");
        assert_eq!((defense.model, defense.action), (41, 43));
        assert_eq!(defense.cave_collision_cooldown, 1);
        assert_eq!(defense.source_word_2e, 0);
        assert_eq!(defense.source_dword_10, 10);
        assert_eq!(defense.construction_target_height, 20);
        assert_eq!(
            defense.source_byte_3e,
            (defense_slot as u8).wrapping_add(1),
            "MSPRD callback epilogue advances byte_0x3E exactly once"
        );
        assert_eq!(session.economy().terrain.planes, before);
    }

    #[test]
    fn level_two_applyevents_terminal_reclaims_castle_children() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("ApplyEvents terminal-reclaim session");
        let baseline_hash = session.economy().reset_hash();
        // ApplyEvents terminally advances/reclaims class-10 models 43 and 79
        // through the same final matrix as models 41/42. Model 79/action
        // 0x56 is a live callback only when the action-table route dispatches
        // that specific action; this direct matrix test uses the terminal
        // action value and proves both source models are covered.
        let models = [15_u8, 41, 42, 43, 79];
        let expected_count = models.len();
        let (slots, free_before) = {
            let economy = session.economy_mut();
            let mut slots = Vec::with_capacity(expected_count);
            for (ordinal, model) in models.iter().copied().enumerate() {
                let source_entity_index = economy
                    .allocate_slot()
                    .expect("ApplyEvents terminal child slot");
                let child = Level2CastleChild {
                    source_entity_index,
                    entity_rng: economy.new_event_rng(source_entity_index),
                    id_entity_index: 111,
                    source_class: if model == 15 { 5 } else { 10 },
                    parent_entity_index: 111,
                    owner_entity_index: 111,
                    player_entity_index: 111,
                    position: Position {
                        x: 0x1000 + (ordinal as u16) * 0x100,
                        y: 0x2000,
                        z: 0,
                    },
                    axis_position: Position::default(),
                    model,
                    action: if model == 15 { 121 } else { 0 },
                    mana: 0,
                    max_mana: 0,
                    life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                    max_life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                    speed: LEVEL2_CASTLE_CHILD_DEFAULT_SPEED,
                    target_sphere_slot: 0,
                    source_byte_3e: source_entity_index as u8,
                    source_byte_3b: 0,
                    cave_collision_cooldown: 0,
                    source_dword_10: 0,
                    source_word_2e: 0,
                    construction_target_height: 0,
                    source_subspell_index: 0,
                    yaw: 0,
                    roll: 0,
                    control_flags: 0,
                    chain_previous_slot: 0,
                    chain_next_slot: 0,
                    source_byte_43: 0,
                    source_word_4a: 0,
                    active: true,
                };
                economy.sync_castle_child_map(child);
                slots.push(source_entity_index);
                economy.castle_children.push(child);
            }
            (slots, economy.free_slots.len())
        };
        session
            .economy_mut()
            .castle_children
            .iter_mut()
            .find(|child| child.source_entity_index == slots[0])
            .expect("wrong class/model guard child")
            .source_class = 10;
        assert!(
            session
                .economy_mut()
                .step_castle_child_terminal_slot(slots[0])
                == false
        );
        session
            .economy_mut()
            .castle_children
            .iter_mut()
            .find(|child| child.source_entity_index == slots[0])
            .expect("restore class/model guard child")
            .source_class = 5;
        for (index, slot) in slots.iter().enumerate() {
            let reclaimed = if index == 0 {
                assert!(
                    session.economy_mut().step_castle_child_terminal_slot(*slot),
                    "ApplyEvents disables and reclaims terminal child in one visit"
                );
                None
            } else {
                Some(
                    session
                        .economy_mut()
                        .applyevents_terminal_reclaim_castle_child(*slot)
                        .expect("terminal child reclaim returns source write"),
                )
            };
            if let Some(reclaimed) = reclaimed {
                assert_eq!(
                    reclaimed.source_byte_3e,
                    u8::try_from(*slot)
                        .expect("fixture terminal source slot fits byte_0x3E")
                        .wrapping_add(1),
                    "class-10 terminal row advances byte_0x3E before reclaim"
                );
            }
            assert!(
                session
                    .economy()
                    .castle_children()
                    .iter()
                    .all(|child| child.source_entity_index != *slot)
            );
            assert!(
                !session
                    .economy()
                    .map_chains()
                    .linked_entity_indices()
                    .contains(slot)
            );
            assert_eq!(session.economy().free_slots.len(), free_before + index + 1);
        }
        assert!(
            session
                .economy_mut()
                .step_castle_child_terminal_slot(slots[0])
                == false
        );
        assert!(session.economy().castle_children().is_empty());
        for slot in &slots {
            assert_eq!(session.economy_mut().allocate_slot(), Some(*slot));
        }
        assert_eq!(session.economy().free_slots.len(), free_before);
        assert_ne!(session.economy().reset_hash(), baseline_hash);
        session.reset();
        assert_eq!(session.economy().reset_hash(), baseline_hash);
    }

    #[test]
    fn level_two_outer_turn_reclaims_model15_auxiliary_through_source_no_call_route() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("model-15 outer-dispatch session");
        // Materialise the authored group-8 constructor before allocating a
        // test Castle, so the test's free-stack assertions isolate the
        // Castle lifecycle rather than same-turn authored construction.
        session.step_outer_turn(PlayerCommand::default(), 0);
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("model-15 source Castle");
        session
            .economy_mut()
            .castle
            .as_mut()
            .expect("model-15 Castle")
            .level = 3;
        session.economy_mut().maintain_castle_defenders(castle_slot);
        let auxiliary_slot = session
            .economy()
            .castle()
            .expect("model-15 Castle state")
            .auxiliary_slots[0];
        assert_ne!(auxiliary_slot, 0);
        let free_before = session.economy().free_slots.len();

        // ApplyEvents_498A0 does not call the action table for class-5 rows:
        // it disables drawing and reclaims the model-15 row in this visit.
        session.step_outer_turn(PlayerCommand::default(), 0);

        assert!(
            session
                .economy()
                .castle_children()
                .iter()
                .all(|child| child.source_entity_index != auxiliary_slot)
        );
        assert!(
            !session
                .economy()
                .map_chains()
                .linked_entity_indices()
                .contains(&auxiliary_slot)
        );
        assert_eq!(session.economy().free_slots.len(), free_before + 1);
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::DynamicArena {
                    source_slot,
                    kind: Level2DynamicArenaKind::CastleAuxiliary,
                    ..
                } if *source_slot == auxiliary_slot
            )
        }));
    }

    #[test]
    fn level_two_castle_auxiliary_creation_sets_source_relation_and_cooldown() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("auxiliary creation session");
        let root_position = Position {
            x: 0x20_00,
            y: 0x30_00,
            z: 0,
        };
        let castle_slot = session
            .economy_mut()
            .build_castle(root_position, 111)
            .expect("auxiliary creation Castle");
        session
            .economy_mut()
            .castle
            .as_mut()
            .expect("auxiliary creation Castle")
            .level = 3;
        session.economy_mut().maintain_castle_defenders(castle_slot);

        let castle = session.economy().castle().expect("auxiliary Castle");
        assert_eq!(castle.auxiliary_count, 4);
        assert_eq!(castle.auxiliary_cooldown, 16);
        let auxiliary_slot = castle.auxiliary_slots[0];
        assert_ne!(auxiliary_slot, 0);
        assert_eq!(&castle.auxiliary_slots[1..4], &[0, 0, 0]);
        let child = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == auxiliary_slot)
            .expect("created model-15 auxiliary child");
        assert_eq!(
            (
                child.source_class,
                child.model,
                child.action,
                child.id_entity_index,
                child.player_entity_index,
                child.yaw,
                child.roll
            ),
            (
                5,
                15,
                121,
                castle.source_id_entity_index,
                castle.source_id_entity_index,
                512,
                512
            )
        );
        assert_eq!(
            child.position,
            Position {
                x: root_position.x.wrapping_add(128),
                y: root_position.y.wrapping_add(640),
                z: session.economy().terrain.ground_height(Position {
                    x: root_position.x.wrapping_add(128),
                    y: root_position.y.wrapping_add(640),
                    z: 0,
                }),
            }
        );
    }

    #[test]
    fn level_two_castle_auxiliary_invalid_relation_clears_slot_and_restarts_cooldown() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("auxiliary invalidation session");
        let baseline_hash = session.economy().reset_hash();
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("auxiliary invalidation Castle");
        let invalid_slot;
        {
            let economy = session.economy_mut();
            economy
                .castle
                .as_mut()
                .expect("auxiliary invalidation Castle")
                .level = 3;
            economy.maintain_castle_defenders(castle_slot);
            invalid_slot = economy
                .castle
                .as_ref()
                .expect("created auxiliary Castle")
                .auxiliary_slots[0];
            economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == invalid_slot)
                .expect("created auxiliary child")
                .action = 125;
            economy
                .castle
                .as_mut()
                .expect("auxiliary invalidation Castle")
                .auxiliary_cooldown = 0;
            economy.maintain_castle_defenders(castle_slot);
        }
        let castle = session.economy().castle().expect("invalidated Castle");
        assert_eq!(castle.auxiliary_slots[0], 0);
        assert_eq!(castle.auxiliary_count, 4);
        assert_eq!(castle.auxiliary_cooldown, 16);
        // `sub_5FF50` clears only the stale relation/cooldown. The wrong
        // action row remains live and is not returned to the free arena by
        // this maintenance walk.
        assert!(!session.economy().free_slots.contains(&invalid_slot));
        assert!(
            session
                .economy()
                .map_chains()
                .linked_entity_indices()
                .contains(&invalid_slot)
        );
        assert!(
            session
                .economy()
                .castle_children()
                .iter()
                .find(|child| child.source_entity_index == invalid_slot)
                .is_some_and(|child| child.active && child.action == 125)
        );
        assert_ne!(session.economy().reset_hash(), baseline_hash);
        session.reset();
        assert_eq!(session.economy().reset_hash(), baseline_hash);
    }

    #[test]
    fn level_two_castle_auxiliary_walk_respects_authored_tail_bound() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("auxiliary tail-bound session");
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("auxiliary tail-bound Castle");
        {
            let economy = session.economy_mut();
            let castle = economy
                .castle
                .as_mut()
                .expect("auxiliary tail-bound Castle");
            castle.level = 3;
            castle.auxiliary_slots[4] = 0xbeef;
            castle.auxiliary_count = 0;
            castle.auxiliary_cooldown = 16;
            economy.maintain_castle_defenders(castle_slot);
        }
        let castle = session.economy().castle().expect("tail-bound Castle");
        assert_eq!(castle.auxiliary_cooldown, 15);
        assert_eq!(castle.auxiliary_slots[4], 0xbeef);
    }

    #[test]
    fn level_two_owner_mana_projection_matches_source_filters_and_reset() {
        let rows = vec![
            Level2OwnerManaRow {
                source_entity_index: 1,
                source_class: 3,
                source_model: 2,
                player_entity_index: 111,
                mana: 7,
                active: true,
            },
            Level2OwnerManaRow {
                source_entity_index: 2,
                source_class: 3,
                source_model: 3,
                player_entity_index: 111,
                mana: 11,
                active: true,
            },
            Level2OwnerManaRow {
                source_entity_index: 3,
                source_class: 5,
                source_model: 15,
                player_entity_index: 111,
                mana: 13,
                active: true,
            },
            Level2OwnerManaRow {
                source_entity_index: 4,
                source_class: 10,
                source_model: 39,
                player_entity_index: 111,
                mana: 17,
                active: true,
            },
            Level2OwnerManaRow {
                source_entity_index: 5,
                source_class: 10,
                source_model: 45,
                player_entity_index: 111,
                mana: 19,
                active: true,
            },
            Level2OwnerManaRow {
                source_entity_index: 6,
                source_class: 10,
                source_model: 58,
                player_entity_index: 111,
                mana: 23,
                active: true,
            },
            // A zero player relation still contributes to the source global
            // counter but cannot enter an owner's dword_0x13C bucket.
            Level2OwnerManaRow {
                source_entity_index: 7,
                source_class: 10,
                source_model: 45,
                player_entity_index: 0,
                mana: 29,
                active: true,
            },
            // Unsupported class/model rows are skipped entirely.
            Level2OwnerManaRow {
                source_entity_index: 8,
                source_class: 10,
                source_model: 40,
                player_entity_index: 111,
                mana: 31,
                active: true,
            },
            Level2OwnerManaRow {
                source_entity_index: 9,
                source_class: 3,
                source_model: 2,
                player_entity_index: 111,
                mana: 37,
                active: false,
            },
        ];
        let (aggregate, global) = level2_owner_mana_projection(&rows);
        assert_eq!(aggregate.get(&111), Some(&19));
        assert_eq!(aggregate.get(&0), None);
        assert_eq!(global, 1 + 7 + 11 + 13 + 17 + 19 + 23 + 29);

        // `sub_61000` feeds every qualifying row into the owner's maxMana;
        // model-39 is deliberately distinct from the model-45 aggregate
        // assertion above.
        let baseline = BTreeMap::from([(111_u16, 100_i32), (222_u16, 50_i32)]);
        let projected = level2_owner_max_mana_projection(&baseline, &rows);
        assert_eq!(projected.get(&111), Some(&190));
        assert_eq!(projected.get(&222), Some(&50));

        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("owner aggregate session");
        assert_eq!(session.economy().owner_mana_global(), 12_835);
        assert_eq!(session.economy().owner_mana_aggregate(111), 0);
        let baseline_hash = session.economy().reset_hash();

        session.reset();
        assert_eq!(session.economy().reset_hash(), baseline_hash);
        assert_eq!(session.economy().owner_mana_aggregate(111), 0);
        assert_eq!(session.economy().owner_mana_global(), 12_835);
    }

    #[test]
    fn level_two_house_possession_reaches_castle_projection_at_next_source_boundary() {
        let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        let inspection = NativeWorld::inspect_level2_with_owner_cache(
            &level_two_fixture_path(),
            &gameplay_cache,
        )
        .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("qualified Level 2 production session");
        session.select_level2_campaign_entry_stage();
        assert_eq!(session.economy().player_mana(), 1_000);

        let live_house_slots = session
            .ordinary()
            .houses()
            .iter()
            .filter(|house| house.active)
            .map(|house| house.entity_index)
            .collect::<Vec<_>>();
        assert_eq!(live_house_slots, (1_u16..=22).collect::<Vec<_>>());
        let house_slots = session
            .ordinary()
            .houses()
            .iter()
            .filter(|house| house.active && !house.parameter.possession_blocked())
            .map(|house| house.entity_index)
            .collect::<Vec<_>>();
        let house_mana = session
            .economy()
            .baseline_owner_mana_rows
            .iter()
            .filter(|row| {
                house_slots.contains(&row.source_entity_index)
                    && row.source_class == HOUSE_CLASS
                    && row.source_model == HOUSE_MODEL
                    && row.active
            })
            .map(|row| row.mana)
            .sum::<i32>();
        assert_eq!(house_mana, 2_034);
        assert_eq!(session.economy().owner_mana_aggregate(111), 0);

        let castle_position = Position::default();
        let sphere_position = Position {
            x: 10_000,
            y: 10_000,
            z: 0,
        };
        let loaded_sphere_id = session
            .economy_mut()
            .spawn_mana_sphere_from_death(901, sphere_position, 0, 0x4321, 7_000, 0)
            .expect("qualified loading sphere");
        let loaded_sphere_slot = u16::try_from(loaded_sphere_id).expect("loading sphere slot");
        assert!(
            session
                .economy_mut()
                .claim_mana_sphere(loaded_sphere_slot, 111)
        );
        let carrier_slot = session
            .economy_mut()
            .spawn_mana_carrier(sphere_position, 111, loaded_sphere_id)
            .expect("qualified loading carrier");
        let castle_slot = session
            .economy_mut()
            .build_castle(castle_position, 111)
            .expect("qualified level-one Castle");
        let castle = session.economy().castle().expect("loaded Castle");
        assert_eq!(
            (castle.level, castle.mana, castle.mana_capacity),
            (1, 0, 8_500)
        );
        assert_eq!(castle.phase & 1, (castle_slot as u8) & 1);
        assert!(
            session
                .economy_mut()
                .absorb_sphere_into_carrier(loaded_sphere_slot, carrier_slot)
        );
        assert!(session.economy_mut().return_carrier_to_castle(carrier_slot));
        assert!(
            session
                .economy_mut()
                .step_carrier_to_castle(carrier_slot, castle_position)
        );
        assert_eq!(
            session.economy().castle().expect("loaded Castle").mana,
            7_000
        );
        let loaded_castle = session.economy().castle().expect("loaded Castle");
        assert!(
            session
                .economy()
                .player_mana()
                .saturating_add(loaded_castle.mana)
                < loaded_castle.mana_capacity
        );
        assert!(house_mana.saturating_add(loaded_castle.mana) > loaded_castle.mana_capacity);

        for &house_slot in &house_slots {
            session
                .ordinary_mut()
                .submit_house_possession(house_slot, 111, 1)
                .expect("qualified House possession mailbox");
        }

        let initial_castle_phase = session.economy().castle().expect("initial Castle").phase;
        assert_eq!(initial_castle_phase & 1, (castle_slot as u8) & 1);
        assert_ne!(initial_castle_phase & 1, 0);

        session.step_outer_turn(PlayerCommand::default(), 0);
        let possessed_slots = session
            .dispatch_events()
            .iter()
            .flat_map(|event| match event {
                Level2DispatchEvent::House { events, .. } => events
                    .iter()
                    .filter_map(|event| match event {
                        HouseStepEvent::Possessed {
                            entity_index,
                            owner_entity_index,
                            ..
                        } => Some((*entity_index, *owner_entity_index)),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
                _ => Vec::new(),
            })
            .map(|(source_slot, owner)| {
                assert_eq!(owner, 111);
                source_slot
            })
            .collect::<Vec<_>>();
        assert_eq!(possessed_slots, house_slots);
        // House mailboxes are consumed by this cursor, but sub_60F00's
        // source-boundary projection remains frozen for the rest of the pass.
        assert_eq!(session.economy().owner_mana_aggregate(111), 0);
        let first_castle = session.economy().castle().expect("first-turn Castle");
        assert_eq!(first_castle.phase & 1, 0);
        let owner_max_before_model39 = session
            .economy()
            .owner_max_mana
            .get(&111)
            .copied()
            .expect("player max-mana projection");
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Castle { source_slot, .. }
                    if *source_slot == castle_slot
            )
        }));

        let global_before_model39 = session.economy().owner_mana_global();
        let model39_id = session
            .economy_mut()
            .spawn_mana_sphere_from_death(902, sphere_position, 0, 0x5678, 1, 111)
            .expect("qualified owned model-39 negative sphere");
        let model39_slot = u16::try_from(model39_id).expect("model-39 sphere slot");
        assert_eq!(
            session.economy().owner_mana_global(),
            global_before_model39,
            "a same-pass model-39 spawn cannot publish sub_60F00's global counter early"
        );
        assert_eq!(session.economy().owner_mana_aggregate(111), 0);
        assert_eq!(
            session.economy().owner_max_mana.get(&111),
            Some(&owner_max_before_model39)
        );

        session.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(session.economy().owner_mana_aggregate(111), house_mana);
        assert_eq!(
            session.economy().owner_max_mana.get(&111),
            Some(&(owner_max_before_model39 + house_mana + 1))
        );
        assert_eq!(
            session.economy().owner_mana_global(),
            global_before_model39 + 1
        );
        let castle = session.economy().castle().expect("second-turn Castle");
        assert_eq!(castle.mana, 7_000);
        assert_eq!(castle.defender_count, 1);
        let defender_slot = castle.defender_slots[0];
        let defender = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defender_slot)
            .copied()
            .expect("ordinary Castle defender");
        assert_eq!(
            (defender.source_class, defender.model, defender.action),
            (3, 3, 9)
        );
        assert_eq!(defender.target_sphere_slot, castle_slot);
        assert_ne!(defender.target_sphere_slot, model39_slot);
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Castle { source_slot, .. }
                    if *source_slot == castle_slot
            )
        }));
        let castle_dispatch_index = session
            .dispatch_events()
            .iter()
            .position(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::Castle { source_slot, .. }
                        if *source_slot == castle_slot
                )
            })
            .expect("ordinary Castle dispatch");
        let defender_dispatch_index = session
            .dispatch_events()
            .iter()
            .position(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::DynamicArena {
                        source_slot,
                        kind: Level2DynamicArenaKind::CastleDefender,
                        ..
                    } if *source_slot == defender_slot
                )
            })
            .expect("ordinary Castle defender dispatch");
        assert!(castle_dispatch_index < defender_dispatch_index);

        session.reset();
        session.select_level2_campaign_entry_stage();
        let positive_house_slots = session
            .economy()
            .baseline_owner_mana_rows
            .iter()
            .filter(|row| row.source_class == HOUSE_CLASS && row.source_model == HOUSE_MODEL)
            .filter(|row| row.mana > 0)
            .map(|row| row.source_entity_index)
            .collect::<BTreeSet<_>>();
        let disabled_house = session
            .ordinary()
            .houses()
            .iter()
            .copied()
            .find(|house| house.active && positive_house_slots.contains(&house.entity_index))
            .expect("qualified positive-mana House for disable path");
        let disabled_mana = session
            .economy()
            .baseline_owner_mana_rows
            .iter()
            .find(|row| row.source_entity_index == disabled_house.entity_index)
            .expect("disabled House owner row")
            .mana;
        let baseline_global = session.economy().owner_mana_global();
        let baseline_campaign_hash = session.campaign_state_hash();
        session
            .ordinary_mut()
            .inject_house_damage(
                disabled_house.entity_index,
                disabled_house.life.wrapping_add(1),
                111,
            )
            .expect("qualified House destruction mailbox");
        session.step_outer_turn(PlayerCommand::default(), 0);
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::House { events, .. }
                    if events.iter().any(|event| matches!(
                        event,
                        HouseStepEvent::RemoveFootprint { entity_index, .. }
                            if *entity_index == disabled_house.entity_index
                    ))
            )
        }));
        assert!(
            !session
                .ordinary()
                .houses()
                .iter()
                .find(|house| house.entity_index == disabled_house.entity_index)
                .expect("disabled House state")
                .active
        );
        assert_eq!(
            session.economy().source_owner(disabled_house.entity_index),
            0
        );
        assert!(
            !session
                .economy()
                .map_chains()
                .is_linked(disabled_house.entity_index)
        );
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .economy()
                .owner_mana_rows
                .iter()
                .any(|row| row.source_entity_index == disabled_house.entity_index)
        );
        assert_eq!(
            session.economy().owner_mana_global(),
            baseline_global - disabled_mana
        );
        assert_ne!(session.campaign_state_hash(), baseline_campaign_hash);
        session.reset();
        assert_eq!(session.campaign_state_hash(), baseline_campaign_hash);
        assert_eq!(session.economy().owner_mana_global(), baseline_global);
        assert!(
            session
                .economy()
                .owner_mana_rows
                .iter()
                .any(|row| row.source_entity_index == disabled_house.entity_index)
        );
    }

    #[test]
    fn level_two_sub5f810_uses_xyz_suppression_relation_exclusion_and_source_order() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("sub5F810 target session");
        let economy = session.economy_mut();
        let holder_position = Position {
            x: 100,
            y: 200,
            z: 300,
        };
        let spawn = |economy: &mut Level2EconomyState, position: Position, owner: u16| {
            economy
                .spawn_mana_sphere_from_death(901, position, 0, 0x1234, 1, owner)
                .expect("target sphere") as u16
        };
        let suppressed = spawn(
            economy,
            Position {
                x: 100,
                y: 200,
                z: 301,
            },
            111,
        );
        let tied_first = spawn(
            economy,
            Position {
                x: 110,
                y: 200,
                z: 300,
            },
            111,
        );
        let tied_second = spawn(
            economy,
            Position {
                x: 90,
                y: 200,
                z: 300,
            },
            111,
        );
        let wrong_player = spawn(
            economy,
            Position {
                x: 101,
                y: 200,
                z: 300,
            },
            112,
        );
        let excluded = spawn(
            economy,
            Position {
                x: 100,
                y: 200,
                z: 300,
            },
            111,
        );
        economy
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == suppressed)
            .expect("suppressed sphere")
            .control_flags = LEVEL2_MANA_SPHERE_TARGET_SUPPRESSION_BIT;
        economy
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == wrong_player)
            .expect("wrong-player sphere")
            .player_entity_index = 112;
        economy.spheres.reverse();
        let expected_tie = tied_first.min(tied_second);
        assert_eq!(
            economy.select_castle_defender_sphere(holder_position, 111, excluded, 0),
            Some(expected_tie)
        );
        assert_eq!(
            economy.select_castle_defender_sphere(holder_position, 111, expected_tie, 0),
            Some(excluded)
        );
        let other_tie = if expected_tie == tied_first {
            tied_second
        } else {
            tied_first
        };
        // Excluding both the first tie winner and the exact-position row
        // leaves the other equal-distance source-order candidate.
        assert_eq!(
            economy.select_castle_defender_sphere(holder_position, 111, expected_tie, excluded),
            Some(other_tie)
        );
    }

    #[test]
    fn level_two_reset_projects_static_model39_rows_into_sub5f810_selector() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut entities = inspection.entities().to_vec();
        let source_relation = 111_u16;
        let static_position = entities[1].position;

        // The qualified fixture has no authored model-39 row yet. Reuse
        // active source slots so this test exercises the same from-parts
        // projection against lossless entity records, rather than inserting
        // a sphere directly into the typed economy vector.
        let mut static_entity = entities[1].clone();
        static_entity.class = 10;
        static_entity.model = 39;
        static_entity.entity_rng = 0x1357;
        static_entity.position = static_position;
        static_entity.yaw = -321;
        static_entity.mana = 321;
        static_entity.control_flags = 0;
        static_entity = static_entity
            .with_record_byte(152, source_relation as u8)
            .with_record_byte(153, (source_relation >> 8) as u8);
        entities[1] = static_entity;

        let player_entity = &entities[usize::from(inspection.players()[0].entity_index)];
        let player_baseline_max_mana = entities[usize::from(source_relation)].max_mana;
        let mut economy = Level2EconomyState::from_parts(
            SessionTerrain::from_inspection(&inspection),
            &entities,
            player_entity.mana,
            player_entity.model,
            0,
            inspection.players()[0].enabled_spell_holders[2],
            inspection.runtime().random_state,
        )
        .expect("static model-39 economy projection");

        assert_eq!(economy.spheres().len(), 1);
        let sphere = economy.spheres()[0];
        assert_eq!(
            (
                sphere.id,
                sphere.source_entity_index,
                sphere.entity_rng,
                sphere.position,
                sphere.yaw,
                sphere.mana,
                sphere.control_flags,
                sphere.owner_entity_index,
                sphere.player_entity_index,
            ),
            (
                1,
                1,
                0x1357,
                static_position,
                -321,
                321,
                0,
                source_relation,
                source_relation,
            )
        );
        assert_eq!(
            economy.select_castle_defender_sphere(static_position, source_relation, 0, 0),
            Some(1)
        );
        assert_eq!(
            economy.owner_max_mana.get(&source_relation),
            Some(&(player_baseline_max_mana + 321)),
            "sub_61000 adds static model-39 mana to the linked player's maxMana"
        );

        let baseline_hash = economy.reset_hash();
        economy.spheres[0].mana = 999;
        assert_ne!(economy.reset_hash(), baseline_hash);
        economy.reset();
        assert_eq!(economy.spheres().len(), 1);
        assert_eq!(economy.spheres()[0].mana, 321);
        assert_eq!(economy.reset_hash(), baseline_hash);
    }

    #[test]
    fn level_two_castle_defender_arbitration_tracks_nearest_sphere_and_aggregate() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("defender arbitration session");
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("defender arbitration Castle");
        {
            let economy = session.economy_mut();
            let castle = economy
                .castle
                .as_mut()
                .expect("defender arbitration Castle");
            castle.level = 1;
            castle.phase = 1;
            castle.mana = 0;
            castle.mana_capacity = 10_000;
        }
        session.economy_mut().maintain_castle_defenders(castle_slot);
        let defender_slot = session
            .economy()
            .castle()
            .expect("defender arbitration Castle")
            .defender_slots[0];
        {
            let economy = session.economy_mut();
            let defender = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("model-3 defender");
            defender.mana = 7;
            defender.max_mana = 100;
        }
        let sphere_id = session
            .economy_mut()
            .spawn_mana_sphere_from_death(
                77,
                Position {
                    x: 256,
                    y: 128,
                    z: 0,
                },
                0,
                0x1234,
                50,
                111,
            )
            .expect("nearest defender sphere");
        session.economy_mut().maintain_castle_defenders(castle_slot);
        let castle = session
            .economy()
            .castle()
            .expect("defender arbitration Castle");
        assert_eq!(
            (castle.defender_total_mana, castle.defender_total_max_mana),
            (7, 100)
        );
        assert_eq!(
            castle.defender_target_spheres[0],
            u16::try_from(sphere_id).unwrap()
        );
        let sphere_slot = u16::try_from(sphere_id).unwrap();
        {
            let economy = session.economy_mut();
            economy
                .castle
                .as_mut()
                .expect("defender arbitration Castle")
                .mana_capacity = 10_000;
            let defender = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("model-3 defender");
            defender.mana = defender.max_mana;
            defender.target_sphere_slot = 0;
            economy.maintain_castle_defenders(castle_slot);
        }
        assert_eq!(
            session
                .economy()
                .castle()
                .expect("defender arbitration Castle")
                .defender_target_spheres[0],
            castle_slot
        );
        assert_eq!(
            session
                .economy()
                .castle()
                .expect("defender arbitration Castle")
                .defender_total_mana,
            100
        );
        assert!(
            session
                .economy()
                .spheres()
                .iter()
                .any(|sphere| sphere.source_entity_index == sphere_slot)
        );
    }

    #[test]
    fn level_two_castle_defender_cadence_keeps_fixed_source_v21_after_dead_slot() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("fixed-v21 defender cadence session");
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("fixed-v21 defender cadence Castle");
        {
            let castle = session.economy_mut().castle.as_mut().expect("Castle");
            castle.level = 4; // sub_60400 returns fixed v21 = 2.
            castle.phase = 0;
            castle.mana = 0;
            castle.mana_capacity = 78_600;
        }
        session.economy_mut().maintain_castle_defenders(castle_slot);
        let (dead_slot, live_slot) = {
            let castle = session.economy().castle().expect("Castle");
            (castle.defender_slots[0], castle.defender_slots[1])
        };
        assert_ne!(dead_slot, 0);
        assert_ne!(live_slot, 0);
        let sentinel_target = 0xdead;
        {
            let economy = session.economy_mut();
            economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == dead_slot)
                .expect("slot-0 defender")
                .life = -1;
            let live = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == live_slot)
                .expect("slot-1 defender");
            live.source_byte_3e = 1;
            live.target_sphere_slot = sentinel_target;
        }

        // The dead source slot is cleared first, but v21 remains two. A
        // compact defenders.len()==1 loop would turn byte_0x3E==1 into a
        // modulo-one hit and overwrite the live slot's sentinel target.
        session.economy_mut().maintain_castle_defenders(castle_slot);
        let live = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == live_slot)
            .copied()
            .expect("live slot-1 defender remains indexed");
        assert_eq!(live.source_byte_3e, 1);
        assert_eq!(live.target_sphere_slot, sentinel_target);
        let castle = session.economy().castle().expect("Castle");
        assert_eq!(castle.defender_slots, [0, live_slot, 0]);
        assert_eq!(castle.defender_count, 1);
    }

    #[test]
    fn level_two_castle_outer_turn_sub5ff50_transforms_dead_excess_and_reclaims_following_pass() {
        let package = level_two_fixture_path();
        let inspection = NativeWorld::inspect_level2(&package).expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("qualified Level 2 reset session");
        let baseline_hash = session.economy().reset_hash();
        let _castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("sub_5FF50 Castle");
        {
            let castle = session.economy_mut().castle.as_mut().expect("Castle");
            castle.level = 1;
            castle.phase = 0;
            castle.mana = 0;
            castle.mana_capacity = 5_000;
        }
        session.select_level2_campaign_entry_stage();
        session.step_outer_turn(PlayerCommand::default(), 0);
        let defender_slot = session.economy().castle().expect("Castle").defender_slots[0];
        assert_ne!(defender_slot, 0);
        let dead_position = Position {
            x: 4_096,
            y: 0,
            z: 0,
        };
        let dead_rng = {
            let economy = session.economy_mut();
            let defender = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("live Castle defender");
            defender.life = -1;
            defender.mana = 321;
            defender.position = dead_position;
            defender.target_sphere_slot = 0;
            defender.entity_rng
        };
        session.economy_mut().castle.as_mut().expect("Castle").phase = 0;
        let global_before_death = session.economy().global_rng;
        let free_before_death = session.economy().free_slot_count();

        // The root's ordinary action-4 callback reaches sub_5FF50. A valid
        // dead holder becomes one normal TransformEntityToManaSphere(false)
        // sphere, clears its raw relation, and remains hidden but allocated
        // for the rest of this source pass.
        session.step_outer_turn(PlayerCommand::default(), 0);
        let hidden = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defender_slot)
            .copied()
            .expect("dead defender remains until following source boundary");
        assert!(hidden.active);
        assert_ne!(hidden.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
        assert_eq!(hidden.player_entity_index, 0);
        assert_eq!(hidden.entity_rng, level2_next_rng16(dead_rng));
        assert!(!session.economy().free_slots.contains(&defender_slot));
        assert!(session.economy().map_chains().is_linked(defender_slot));
        assert_eq!(
            session.economy().free_slot_count(),
            free_before_death.saturating_sub(1),
            "the transformed sphere consumes one slot while the hidden holder remains allocated"
        );
        assert_eq!(session.economy().source_owner(defender_slot), 0);
        assert_eq!(
            session.economy().castle().expect("Castle").defender_slots[0],
            0
        );
        let transformed = session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| {
                sphere.active
                    && sphere.source_entity_index != defender_slot
                    && sphere.owner_entity_index == 111
                    && sphere.mana == 321
            })
            .copied()
            .expect("dead defender mana sphere");
        let global_after_death = level2_next_rng32(global_before_death);
        let sphere_seed = transformed
            .source_entity_index
            .wrapping_add(global_after_death as u16);
        assert_eq!(
            transformed.entity_rng,
            level2_next_rng16(level2_next_rng16(sphere_seed)),
            "TransformEntityToManaSphere keeps its death-path two-draw sphere RNG"
        );
        assert!(session.economy().events().iter().any(|event| {
            matches!(
                event,
                Level2EconomyEvent::ManaSphereSpawned {
                    source_entity_index,
                    mana: 321,
                    ..
                } if *source_entity_index == defender_slot
            )
        }));

        // Force the next ordinary root visit to the stage-0 target so the
        // following source-boundary reclaim is observed without immediately
        // allocating a replacement holder into the just-freed slot.
        session.economy_mut().castle.as_mut().expect("Castle").level = 0;
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            session
                .economy()
                .castle_children()
                .iter()
                .all(|child| child.source_entity_index != defender_slot)
        );
        assert!(session.economy().free_slots.contains(&defender_slot));
        assert!(!session.economy().map_chains().is_linked(defender_slot));
        assert_ne!(session.economy().reset_hash(), baseline_hash);
        session.reset();
        assert_eq!(session.economy().reset_hash(), baseline_hash);

        // A zero-mana holder takes the same valid dead-holder branch, but the
        // pinned TransformEntityToManaSphere(false) returns before its raw
        // player relation write. The relation therefore remains nonzero for
        // the hidden interval; exact typed/owner/id identity, rather than a
        // cleared relation, drives the following source-boundary reclaim.
        let mut zero_mana = inspection
            .level2_reset_session()
            .expect("qualified zero-mana Level 2 reset session");
        let zero_castle_slot = zero_mana
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("zero-mana Castle");
        {
            let castle = zero_mana.economy_mut().castle.as_mut().expect("Castle");
            castle.level = 1;
            castle.phase = 0;
            castle.mana = 0;
            castle.mana_capacity = 5_000;
        }
        zero_mana.select_level2_campaign_entry_stage();
        zero_mana.step_outer_turn(PlayerCommand::default(), 0);
        let zero_slot = zero_mana
            .economy()
            .castle()
            .expect("zero-mana Castle")
            .defender_slots[0];
        let (zero_rng, zero_player_relation, zero_free_slots, zero_event_count) = {
            let economy = zero_mana.economy_mut();
            let child = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == zero_slot)
                .expect("zero-mana defender");
            child.life = -1;
            child.mana = 0;
            child.position = Position {
                x: 4_096,
                y: 0,
                z: 0,
            };
            child.target_sphere_slot = 0;
            (
                child.entity_rng,
                child.player_entity_index,
                economy.free_slot_count(),
                economy.events.len(),
            )
        };
        assert_ne!(zero_player_relation, 0);
        zero_mana
            .economy_mut()
            .castle
            .as_mut()
            .expect("zero-mana Castle")
            .phase = 0;
        zero_mana.step_outer_turn(PlayerCommand::default(), 0);
        let hidden_zero = zero_mana
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == zero_slot)
            .copied()
            .expect("zero-mana holder remains hidden until following source boundary");
        assert!(hidden_zero.active);
        assert_ne!(hidden_zero.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
        assert_eq!(hidden_zero.player_entity_index, zero_player_relation);
        assert_eq!(hidden_zero.entity_rng, zero_rng);
        assert_eq!(zero_mana.economy().free_slot_count(), zero_free_slots);
        assert!(zero_mana.economy().map_chains().is_linked(zero_slot));
        assert_eq!(
            zero_mana
                .economy()
                .castle()
                .expect("zero-mana Castle")
                .defender_slots[0],
            0
        );
        assert!(
            zero_mana.economy().events()[zero_event_count..]
                .iter()
                .all(|event| {
                    !matches!(
                        event,
                        Level2EconomyEvent::ManaSphereSpawned {
                            source_entity_index,
                            ..
                        } if *source_entity_index == zero_slot
                    )
                })
        );

        zero_mana
            .economy_mut()
            .castle
            .as_mut()
            .expect("zero-mana Castle")
            .level = 0;
        zero_mana.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            zero_mana
                .economy()
                .castle_children()
                .iter()
                .all(|child| child.source_entity_index != zero_slot)
        );
        assert!(zero_mana.economy().free_slots.contains(&zero_slot));
        assert!(!zero_mana.economy().map_chains().is_linked(zero_slot));
        assert!(zero_mana.dispatch_events().iter().any(|event| matches!(
            event,
            Level2DispatchEvent::Castle { source_slot, .. }
                if *source_slot == zero_castle_slot
        )));

        // The unused relation tail takes the same valid-holder transform
        // branch. Build two source defenders through the ordinary root route,
        // then lower the stage so the second relation becomes excess.
        let mut excess = inspection
            .level2_reset_session()
            .expect("qualified Level 2 reset session");
        let excess_castle_slot = excess
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("excess-holder Castle");
        {
            let castle = excess.economy_mut().castle.as_mut().expect("Castle");
            castle.level = 4;
            castle.phase = 0;
        }
        excess.select_level2_campaign_entry_stage();
        excess.step_outer_turn(PlayerCommand::default(), 0);
        let (primary_excess_slot, excess_slot) = {
            let castle = excess.economy().castle().expect("excess-holder Castle");
            (castle.defender_slots[0], castle.defender_slots[1])
        };
        assert_ne!(primary_excess_slot, 0);
        assert_ne!(excess_slot, 0);
        {
            let economy = excess.economy_mut();
            let castle = economy.castle.as_mut().expect("excess-holder Castle");
            castle.level = 1;
            castle.phase = 0;
            let holder = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == excess_slot)
                .expect("valid excess holder");
            holder.mana = 222;
            holder.position = Position {
                x: 4_096,
                y: 0,
                z: 0,
            };
            holder.target_sphere_slot = 0;
        }
        excess.step_outer_turn(PlayerCommand::default(), 0);
        let hidden_excess = excess
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == excess_slot)
            .copied()
            .expect("excess holder remains until following source boundary");
        assert!(hidden_excess.active);
        assert_ne!(hidden_excess.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
        assert_eq!(hidden_excess.player_entity_index, 0);
        assert!(excess.economy().map_chains().is_linked(excess_slot));
        assert_eq!(
            excess
                .economy()
                .castle()
                .expect("excess-holder Castle")
                .defender_slots,
            [primary_excess_slot, 0, 0]
        );
        assert!(excess.economy().events().iter().any(|event| {
            matches!(
                event,
                Level2EconomyEvent::ManaSphereSpawned {
                    source_entity_index,
                    mana: 222,
                    ..
                } if *source_entity_index == excess_slot
            )
        }));
        excess
            .economy_mut()
            .castle
            .as_mut()
            .expect("excess-holder Castle")
            .level = 0;
        excess.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            excess
                .economy()
                .castle_children()
                .iter()
                .all(|child| child.source_entity_index != excess_slot)
        );
        assert!(excess.economy().free_slots.contains(&excess_slot));
        assert!(excess.dispatch_events().iter().any(|event| matches!(
            event,
            Level2DispatchEvent::Castle { source_slot, .. }
                if *source_slot == excess_castle_slot
        )));

        // A wrong relation must not release a live non-defender row. The
        // relation slot is authoritative for selection, but the class/model/
        // action predicate is authoritative for the Transform branch.
        let mut wrong = inspection
            .level2_reset_session()
            .expect("qualified Level 2 reset session");
        let wrong_castle_slot = wrong
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("wrong-relation Castle");
        wrong
            .economy_mut()
            .castle
            .as_mut()
            .expect("wrong-relation Castle")
            .level = 1;
        wrong
            .economy_mut()
            .castle
            .as_mut()
            .expect("wrong-relation Castle")
            .phase = 0;
        wrong.select_level2_campaign_entry_stage();
        wrong.step_outer_turn(PlayerCommand::default(), 0);
        let wrong_slot = wrong
            .economy()
            .castle()
            .expect("wrong-relation Castle")
            .defender_slots[0];
        {
            let economy = wrong.economy_mut();
            let child = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == wrong_slot)
                .expect("wrong-relation holder");
            child.source_class = 10;
            child.model = 42;
            child.action = 44;
            economy
                .castle
                .as_mut()
                .expect("wrong-relation Castle")
                .phase = 0;
        }
        wrong.step_outer_turn(PlayerCommand::default(), 0);
        let wrong_child = wrong
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == wrong_slot)
            .copied()
            .expect("wrongly related live row is preserved");
        assert!(wrong_child.active);
        assert_eq!(
            (
                wrong_child.source_class,
                wrong_child.model,
                wrong_child.action
            ),
            (10, 42, 44)
        );
        assert_ne!(
            wrong
                .economy()
                .castle()
                .expect("wrong-relation Castle")
                .defender_slots[0],
            wrong_slot
        );
        assert!(!wrong.economy().events().iter().any(|event| {
            matches!(
                event,
                Level2EconomyEvent::ManaSphereSpawned {
                    source_entity_index,
                    ..
                } if *source_entity_index == wrong_slot
            )
        }));

        // A stale slot is cleared/replaced without dereferencing or freeing
        // an unrelated live row.
        wrong
            .economy_mut()
            .castle
            .as_mut()
            .expect("stale-relation Castle")
            .defender_slots[0] = 0xdead;
        wrong
            .economy_mut()
            .castle
            .as_mut()
            .expect("stale-relation Castle")
            .phase = 0;
        wrong.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !wrong
                .economy()
                .castle_children()
                .iter()
                .any(|child| child.source_entity_index == 0xdead)
        );
        assert!(
            wrong
                .economy()
                .castle_children()
                .iter()
                .any(|child| child.source_entity_index == wrong_slot && child.active)
        );
        assert!(wrong.dispatch_events().iter().any(|event| matches!(
            event,
            Level2DispatchEvent::Castle { source_slot, .. }
                if *source_slot == wrong_castle_slot
        )));
    }

    #[test]
    fn level_two_castle_defender_addballon_class3_threshold_terrain_and_life() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let setup = || {
            let mut session = inspection
                .level2_reset_session()
                .expect("Castle defender AddBallon session");
            let baseline_hash = session.economy().reset_hash();
            let castle_position = Position::default();
            let castle_slot = session
                .economy_mut()
                .build_castle(castle_position, 111)
                .expect("Castle defender AddBallon Castle");
            session
                .economy_mut()
                .castle
                .as_mut()
                .expect("Castle defender AddBallon Castle")
                .level = 1;
            session.economy_mut().maintain_castle_defenders(castle_slot);
            let defender_slot = session
                .economy()
                .castle()
                .expect("Castle defender AddBallon Castle")
                .defender_slots[0];
            let holder_position = Position {
                x: castle_position.x.wrapping_add(16),
                y: castle_position.y,
                z: 0,
            };
            let holder_ground = session.economy().terrain.ground_height(holder_position);
            (
                session,
                baseline_hash,
                castle_slot,
                defender_slot,
                castle_position,
                holder_position,
                holder_ground,
            )
        };

        // The source distance helper is XY-only. A holder below the local
        // terrain plane still transfers at exactly speed*target-dword10.
        let (
            mut session,
            baseline_hash,
            castle_slot,
            defender_slot,
            castle_position,
            mut holder_position,
            holder_ground,
        ) = setup();
        holder_position.z = holder_ground.saturating_sub(100);
        {
            let economy = session.economy_mut();
            let child = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("model-3 defender");
            child.position = holder_position;
            child.target_sphere_slot = castle_slot;
            child.mana = 7;
            child.life = 1;
        }
        let map_before = session.economy().map_chains().linked_entity_indices();
        let free_before_transfer = session.economy().free_slot_count();
        assert!(
            session
                .economy_mut()
                .step_castle_defender_slot(defender_slot)
        );
        let castle = session.economy().castle().expect("Castle target");
        let child = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("model-3 defender");
        assert_eq!(castle.position, castle_position);
        assert_eq!(castle.mana, 7);
        assert_eq!(
            (child.position.x, child.position.y),
            (castle_position.x, castle_position.y)
        );
        assert_eq!(child.mana, 0);
        assert_eq!(child.player_entity_index, child.id_entity_index);
        assert_eq!(child.life, child.max_life);
        assert_eq!(
            session
                .economy()
                .events()
                .iter()
                .filter(|event| matches!(event, Level2EconomyEvent::CastleDefenderArrived { .. }))
                .count(),
            1
        );
        assert_eq!(
            session.economy().free_slot_count(),
            free_before_transfer,
            "the typed transfer does not allocate or reclaim a source row"
        );
        assert!(map_before.iter().all(|slot| {
            session
                .economy()
                .map_chains()
                .linked_entity_indices()
                .contains(slot)
        }));
        assert_ne!(session.economy().reset_hash(), baseline_hash);
        session.reset();
        assert_eq!(session.economy().reset_hash(), baseline_hash);

        // One unit beyond the pinned source speed range moves the holder and
        // leaves mana/life untouched.
        let (
            mut session,
            _baseline_hash,
            castle_slot,
            defender_slot,
            castle_position,
            mut holder_position,
            holder_ground,
        ) = setup();
        holder_position.x = holder_position.x.wrapping_add(49);
        holder_position.z = holder_ground;
        {
            let economy = session.economy_mut();
            let child = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("model-3 defender");
            child.position = holder_position;
            child.target_sphere_slot = castle_slot;
            child.mana = 7;
            child.life = 1;
        }
        assert!(
            session
                .economy_mut()
                .step_castle_defender_slot(defender_slot)
        );
        let child = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("model-3 defender");
        assert_eq!(child.mana, 7);
        assert_eq!(child.life, 1);
        assert_ne!(child.position.x, holder_position.x);
        assert_eq!(
            session.economy().castle().expect("Castle target").position,
            castle_position
        );

        // At the same XY threshold, a holder above the source's +512 terrain
        // allowance fails the exact altitude gate and does not transfer.
        let (
            mut session,
            _baseline_hash,
            castle_slot,
            defender_slot,
            _castle_position,
            mut holder_position,
            holder_ground,
        ) = setup();
        holder_position.z = holder_ground.saturating_add(513);
        {
            let economy = session.economy_mut();
            let child = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("model-3 defender");
            child.position = holder_position;
            child.target_sphere_slot = castle_slot;
            child.mana = 7;
            child.life = 1;
        }
        assert!(
            session
                .economy_mut()
                .step_castle_defender_slot(defender_slot)
        );
        let child = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("model-3 defender");
        assert_eq!(child.mana, 7);
        assert_eq!(child.life, 1);
        assert_eq!(child.position.x, holder_position.x);
        assert_eq!(child.position.z, holder_ground.saturating_add(509));
    }

    #[test]
    fn level_two_castle_defender_sphere_target_consumes_through_outer_dispatch() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let setup = || {
            let mut session = inspection
                .level2_reset_session()
                .expect("Castle defender sphere session");
            let baseline_hash = session.economy().reset_hash();
            let castle_slot = session
                .economy_mut()
                .build_castle(Position::default(), 111)
                .expect("Castle defender sphere Castle");
            session
                .economy_mut()
                .castle
                .as_mut()
                .expect("Castle defender sphere Castle")
                .level = 1;
            session.economy_mut().maintain_castle_defenders(castle_slot);
            let defender_slot = session
                .economy()
                .castle()
                .expect("Castle defender sphere Castle")
                .defender_slots[0];
            let holder_position = session
                .economy()
                .castle_children()
                .iter()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("Castle defender sphere holder")
                .position;
            let holder = session
                .economy()
                .castle_children()
                .iter()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("Castle defender sphere holder");
            assert_eq!(
                (holder.max_life, holder.max_mana, holder.speed),
                (
                    LEVEL2_CASTLE_DEFENDER_MAX_LIFE,
                    LEVEL2_CASTLE_DEFENDER_MAX_MANA,
                    LEVEL2_CASTLE_DEFENDER_SPEED
                )
            );
            (
                session,
                baseline_hash,
                castle_slot,
                defender_slot,
                holder_position,
            )
        };

        // Positive production route: source arbitration installs the sphere
        // target, then one outer turn visits the sphere and holder in source
        // order.  The holder receives the sphere payload and the sphere is
        // disabled/unlinked before the retained arena is reclaimed.
        let (mut session, baseline_hash, castle_slot, defender_slot, holder_position) = setup();
        let target_position = Position {
            x: holder_position.x.wrapping_add(100),
            ..holder_position
        };
        let sphere_id = session
            .economy_mut()
            .spawn_mana_sphere_from_death(77, target_position, 0, 0x1234, 321, 111)
            .expect("defender target sphere");
        let sphere_slot = u16::try_from(sphere_id).expect("sphere slot fits");
        session
            .economy_mut()
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == sphere_slot)
            .expect("defender target sphere")
            .velocity_x = 0;
        session
            .economy_mut()
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == sphere_slot)
            .expect("defender target sphere")
            .velocity_y = 0;
        session
            .economy_mut()
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == sphere_slot)
            .expect("defender target sphere")
            .vertical_velocity = 0;
        session.economy_mut().maintain_castle_defenders(castle_slot);
        assert_eq!(
            session
                .economy()
                .castle_children()
                .iter()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("defender holder")
                .target_sphere_slot,
            sphere_slot
        );
        session.step_outer_turn(PlayerCommand::default(), 0);
        let child = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("live defender holder");
        assert_eq!(child.mana, 321);
        assert_eq!(child.player_entity_index, child.id_entity_index);
        assert_eq!(child.target_sphere_slot, 0);
        assert_ne!(child.position.x, target_position.x);
        assert!(
            session
                .economy()
                .spheres()
                .iter()
                .all(|sphere| sphere.source_entity_index != sphere_slot)
        );
        assert!(
            !session
                .economy()
                .map_chains()
                .linked_entity_indices()
                .contains(&sphere_slot)
        );
        let economy_index = session
            .dispatch_events()
            .iter()
            .position(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::Economy {
                        event: Level2EconomyEvent::CastleDefenderCollectedSphere {
                            defender_slot: event_defender,
                            sphere_id: event_sphere,
                            mana,
                        },
                        ..
                    } if *event_defender == defender_slot
                        && *event_sphere == sphere_id
                        && *mana == 321
                )
            })
            .expect("defender sphere economy dispatch");
        let defender_index = session
            .dispatch_events()
            .iter()
            .position(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::DynamicArena {
                        source_slot: event_slot,
                        kind: Level2DynamicArenaKind::CastleDefender,
                        ..
                    } if *event_slot == defender_slot
                )
            })
            .expect("defender dynamic dispatch");
        assert!(economy_index < defender_index);
        assert_ne!(session.economy().reset_hash(), baseline_hash);
        session.reset();
        assert_eq!(session.economy().reset_hash(), baseline_hash);

        // Wrong player relation is rejected even when the target slot is
        // installed manually, matching AddBallon's class-10 owner gate.
        let (mut session, _, _, defender_slot, holder_position) = setup();
        let wrong_owner_position = Position {
            x: holder_position.x.wrapping_add(64),
            ..holder_position
        };
        let sphere_id = session
            .economy_mut()
            .spawn_mana_sphere_from_death(77, wrong_owner_position, 0, 0x1234, 321, 222)
            .expect("wrong-owner sphere");
        let sphere_slot = u16::try_from(sphere_id).expect("sphere slot fits");
        session
            .economy_mut()
            .castle_children
            .iter_mut()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("wrong-owner holder")
            .target_sphere_slot = sphere_slot;
        let wrong_owner_before = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("wrong-owner holder")
            .position;
        assert!(
            session
                .economy_mut()
                .step_castle_defender_slot(defender_slot)
        );
        let wrong_owner_after = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("wrong-owner holder");
        assert_eq!(wrong_owner_after.mana, 0);
        assert_eq!(wrong_owner_after.position.x, wrong_owner_before.x);
        assert_eq!(wrong_owner_after.position.y, wrong_owner_before.y);
        assert!(
            session
                .economy()
                .spheres()
                .iter()
                .any(|sphere| sphere.source_entity_index == sphere_slot && sphere.active)
        );
        // The Day attraction threshold is strict on the positive side: a
        // sphere at 1025 source units is not consumed and clears byte-0 bit
        // 0x40 before the holder moves toward it.
        let (mut session, _, _, defender_slot, holder_position) = setup();
        let far_position = Position {
            x: holder_position.x.wrapping_add(1_025),
            ..holder_position
        };
        let sphere_id = session
            .economy_mut()
            .spawn_mana_sphere_from_death(77, far_position, 0, 0x1234, 321, 111)
            .expect("far sphere");
        let sphere_slot = u16::try_from(sphere_id).expect("sphere slot fits");
        session
            .economy_mut()
            .castle_children
            .iter_mut()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("far holder")
            .target_sphere_slot = sphere_slot;
        assert!(
            session
                .economy_mut()
                .step_castle_defender_slot(defender_slot)
        );
        let sphere = session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.source_entity_index == sphere_slot)
            .expect("far sphere remains");
        assert!(sphere.active);
        assert_eq!(sphere.control_flags & 0x40, 0);

        // `sub_106C0` is strict.  Place a size-0 sphere exactly at the
        // defender's X boundary (13 + 272); equality must not consume it.
        let (mut session, _, _, defender_slot, holder_position) = setup();
        let boundary_position = Position {
            x: holder_position.x.wrapping_add(13 + 272),
            ..holder_position
        };
        let sphere_id = session
            .economy_mut()
            .spawn_mana_sphere_from_death(77, boundary_position, 0, 0x1234, 1, 111)
            .expect("boundary sphere");
        let sphere_slot = u16::try_from(sphere_id).expect("sphere slot fits");
        session
            .economy_mut()
            .castle_children
            .iter_mut()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("boundary holder")
            .target_sphere_slot = sphere_slot;
        assert!(
            session
                .economy_mut()
                .step_castle_defender_slot(defender_slot)
        );
        assert!(
            session
                .economy()
                .spheres()
                .iter()
                .any(|sphere| sphere.source_entity_index == sphere_slot && sphere.active)
        );
        assert_eq!(
            session
                .economy()
                .castle_children()
                .iter()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("boundary holder")
                .mana,
            0
        );

        // The source stores Z as signed 16-bit coordinates before the AABB
        // arithmetic.  Equal high-bit coordinates must therefore retain the
        // same overlap result as zero-based coordinates (a raw-u16 compare
        // would incorrectly see a ~65k-unit separation).
        let (mut session, _, _, _defender_slot, holder_position) = setup();
        let sphere_id = session
            .economy_mut()
            .spawn_mana_sphere_from_death(77, holder_position, 0, 0x1234, 1, 111)
            .expect("signed-Z sphere");
        let sphere_slot = u16::try_from(sphere_id).expect("sphere slot fits");
        let sphere = session
            .economy_mut()
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == sphere_slot)
            .expect("signed-Z sphere");
        sphere.position.z = i16::MIN;
        let defender_position = Position {
            z: i16::MIN,
            ..holder_position
        };
        assert!(level2_castle_defender_sphere_overlap(
            defender_position,
            sphere
        ));
    }

    #[test]
    fn level_two_castle_defender_cave_collision_matches_source_control_tail() {
        // The qualified Level 2 package is Day data; this deliberately
        // synthetic Cave-route test exercises the production defender method
        // after swapping only the terrain mode/planes. It is route coverage,
        // not owner-data Cave qualification.
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("Castle defender Cave session");
        let baseline_hash = session.economy().reset_hash();
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("Cave defender Castle");
        session
            .economy_mut()
            .castle
            .as_mut()
            .expect("Cave defender Castle")
            .level = 1;
        session.economy_mut().maintain_castle_defenders(castle_slot);
        let defender_slot = session
            .economy()
            .castle()
            .expect("Cave defender Castle")
            .defender_slots[0];
        let (holder_position, ground) = {
            let child = session
                .economy()
                .castle_children()
                .iter()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("Cave defender holder");
            (
                child.position,
                session.economy().terrain.ground_height(child.position),
            )
        };
        let tile = crate::Tile {
            x: holder_position.x.to_be_bytes()[0],
            y: holder_position.y.to_be_bytes()[0],
        };
        let terrain_index = terrain_index(tile.x, tile.y);

        // Angle bit 8 blocks the holder. The source toggles bit 0, selects
        // speed 48, emits sound 22 on the uncooldown transition, and applies
        // sub_580E0's ground+512 floor after the -16 step.
        {
            let economy = session.economy_mut();
            economy.terrain.map_type = MapType::Cave;
            economy.terrain.planes.angles.fill(0);
            economy.terrain.planes.secondary_heights.fill(255);
            economy.terrain.planes.angles[terrain_index] = 8;
            let child = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("Cave defender holder");
            child.target_sphere_slot = 0;
            child.position = Position {
                z: ground.saturating_add(520),
                ..holder_position
            };
            child.control_flags = 0;
            child.cave_collision_cooldown = 0;
        }
        assert!(
            session
                .economy_mut()
                .step_castle_defender_slot(defender_slot)
        );
        let child = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("blocked Cave defender holder");
        assert_ne!(child.control_flags & 1, 0);
        assert_eq!(child.speed, LEVEL2_CASTLE_DEFENDER_CAVE_BLOCKED_SPEED);
        assert_eq!(
            child.position.z,
            ground + LEVEL2_CASTLE_DEFENDER_CAVE_FLOOR_CLEARANCE
        );
        assert_eq!(
            child.cave_collision_cooldown,
            LEVEL2_CASTLE_DEFENDER_CAVE_SOUND_COOLDOWN
        );
        assert!(session.economy().events().iter().any(|event| matches!(
            event,
            Level2EconomyEvent::CastleDefenderCaveSound {
                defender_slot: event_slot,
                sound_id: LEVEL2_CASTLE_DEFENDER_CAVE_SOUND,
            } if *event_slot == defender_slot
        )));

        // Clearing the angle and using a high ceiling opens the route. A
        // cooldown of one decrements to zero before the transition, so the
        // source emits the same sound and reloads 32 exactly once.
        session.economy_mut().events.clear();
        {
            let economy = session.economy_mut();
            economy.terrain.planes.angles[terrain_index] = 0;
            let child = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("open Cave defender holder");
            child.position.z = ground.saturating_add(700);
            child.control_flags = 1;
            child.cave_collision_cooldown = 1;
        }
        assert!(
            session
                .economy_mut()
                .step_castle_defender_slot(defender_slot)
        );
        let child = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("open Cave defender holder");
        assert_eq!(child.control_flags & 1, 0);
        assert_eq!(child.speed, LEVEL2_CASTLE_DEFENDER_CAVE_OPEN_SPEED);
        assert_eq!(child.position.z, ground + 700 - 16);
        assert_eq!(
            child.cave_collision_cooldown,
            LEVEL2_CASTLE_DEFENDER_CAVE_SOUND_COOLDOWN
        );
        assert!(session.economy().events().iter().any(|event| matches!(
            event,
            Level2EconomyEvent::CastleDefenderCaveSound {
                defender_slot: event_slot,
                sound_id: LEVEL2_CASTLE_DEFENDER_CAVE_SOUND,
            } if *event_slot == defender_slot
        )));

        // The ceiling half of sub_11E70 independently blocks the route when
        // the angle flag is clear.
        session.economy_mut().events.clear();
        {
            let economy = session.economy_mut();
            economy.terrain.planes.secondary_heights.fill(20);
            let child = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("ceiling-blocked Cave defender holder");
            child.position.z = ground.saturating_add(520);
            child.control_flags = 0;
            child.cave_collision_cooldown = 0;
        }
        assert!(
            session
                .economy_mut()
                .step_castle_defender_slot(defender_slot)
        );
        let child = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("ceiling-blocked Cave defender holder");
        assert_ne!(child.control_flags & 1, 0);
        assert_eq!(child.speed, LEVEL2_CASTLE_DEFENDER_CAVE_BLOCKED_SPEED);

        assert_ne!(session.economy().reset_hash(), baseline_hash);
        session.reset();
        assert_eq!(session.economy().reset_hash(), baseline_hash);
    }

    #[test]
    fn level_two_castle_teardown_without_free_arena_only_restores_action() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("no-free teardown session");
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("no-free teardown Castle");
        let before_sphere_count = session.economy().spheres().len();
        let before_linked_entities = session.economy().map_chains().linked_entity_indices();
        let before_ground = session.economy().terrain.planes.ground_heights.clone();
        let before_rng = session
            .economy()
            .source_rng
            .get(&castle_slot)
            .copied()
            .expect("live Castle source RNG");
        {
            let economy = session.economy_mut();
            economy.free_slots.clear();
            let castle = economy.castle.as_mut().expect("no-free teardown Castle");
            castle.action = 6;
            castle.level = 2;
        }
        assert!(session.economy_mut().step_castle_action(castle_slot));
        let castle = session.economy().castle().expect("gated Castle");
        assert_eq!((castle.action, castle.level), (4, 2));
        assert_eq!(
            session.economy().source_rng.get(&castle_slot).copied(),
            Some(before_rng)
        );
        assert_eq!(
            session.economy().spheres().len(),
            before_sphere_count,
            "no-free teardown cannot manufacture an overflow sphere"
        );
        assert_eq!(
            session.economy().map_chains().linked_entity_indices(),
            before_linked_entities,
            "no-free teardown cannot mutate the source map arena"
        );
        assert_eq!(
            session.economy().terrain.planes.ground_heights,
            before_ground
        );
        assert!(
            !session
                .economy()
                .events()
                .iter()
                .any(|event| matches!(event, Level2EconomyEvent::CastleStageDestroyed { .. }))
        );
    }

    #[test]
    fn level_two_castle_teardown_and_allocator_failure_are_exact() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("no-free teardown session");
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("no-free teardown Castle");
        let before_ground = session.economy().terrain.planes.ground_heights.clone();
        let before_rng = session
            .economy()
            .source_rng
            .get(&castle_slot)
            .copied()
            .expect("live Castle source RNG");
        let before_phase = session.economy().castle().expect("Castle").phase;
        {
            let economy = session.economy_mut();
            economy.free_slots.clear();
            let castle = economy.castle.as_mut().expect("no-free teardown Castle");
            castle.action = 6;
            castle.level = 2;
        }
        session.select_level2_campaign_entry_stage();
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Castle { source_slot, .. }
                    if *source_slot == castle_slot
            )
        }));
        let castle = session.economy().castle().expect("gated Castle");
        assert_eq!((castle.action, castle.level), (4, 2));
        assert_eq!(castle.phase, before_phase.wrapping_add(1));
        assert_eq!(
            session.economy().source_rng.get(&castle_slot).copied(),
            Some(before_rng)
        );
        assert_eq!(
            session.economy().terrain.planes.ground_heights,
            before_ground
        );
        assert!(!session.economy().events().iter().any(|event| {
            matches!(
                event,
                Level2EconomyEvent::CastleDestructionSound { source_entity_index, .. }
                    | Level2EconomyEvent::CastleStageDestroyed { source_entity_index, .. }
                    | Level2EconomyEvent::CastleDestroyed { source_entity_index, .. }
                    if *source_entity_index == castle_slot
            )
        }));

        // `sub_49F90` rebuilds the free arena when an inactive typed row was
        // omitted from the fast free stack.  The retry must reuse that row,
        // keep the live Castle distinct, and leave exactly one committed
        // overflow sphere rather than leaking or colliding with a slot.
        let mut rebuild = inspection
            .level2_reset_session()
            .expect("allocator-rebuild session");
        let rebuild_castle_slot = rebuild
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("allocator-rebuild Castle");
        let stale_slot = {
            let economy = rebuild.economy_mut();
            let stale_id = economy
                .spawn_mana_sphere_from_death(52, Position::default(), 0, 0x9876, 1, 111)
                .expect("stale typed sphere");
            let stale_slot = u16::try_from(stale_id).expect("stale sphere source slot");
            let stale = economy
                .spheres
                .iter_mut()
                .find(|sphere| sphere.source_entity_index == stale_slot)
                .expect("stale typed sphere row");
            stale.active = false;
            let stale_position = stale.position;
            economy
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: stale_slot,
                    active: false,
                    x: stale_position.x,
                    y: stale_position.y,
                })
                .expect("unlink stale typed sphere");
            stale_slot
        };
        {
            let economy = rebuild.economy_mut();
            economy.free_slots.clear();
            let castle = economy.castle.as_mut().expect("allocator-rebuild Castle");
            castle.mana = 2_000;
            castle.mana_capacity = 1_000;
            castle.action = 4;
            castle.phase = 0;
        }
        rebuild.select_level2_campaign_entry_stage();
        rebuild.step_outer_turn(PlayerCommand::default(), 0);
        assert!(rebuild.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Castle { source_slot, .. }
                    if *source_slot == rebuild_castle_slot
            )
        }));
        assert_ne!(stale_slot, rebuild_castle_slot);
        assert!(
            rebuild
                .economy()
                .spheres
                .iter()
                .any(|sphere| sphere.active && sphere.source_entity_index == stale_slot)
        );
        assert!(
            rebuild.economy().map_chains().is_linked(stale_slot),
            "sub_5FD00 CopyEntityPosition relinks a rebuilt overflow sphere"
        );
        assert!(!rebuild.economy().free_slots.contains(&stale_slot));
        assert!(!rebuild.economy().free_slots.contains(&rebuild_castle_slot));
        assert_eq!(
            rebuild
                .economy()
                .spheres
                .iter()
                .filter(|sphere| sphere.active && sphere.source_entity_index == stale_slot)
                .count(),
            1
        );

        // `sub_613D0` follows only the old controller successor chain.  A
        // planted defender and an auxiliary tail row with the same parent
        // must survive into the existing post-downgrade maintenance walk.
        let mut chain = inspection
            .level2_reset_session()
            .expect("controller-chain teardown session");
        chain.select_level2_campaign_entry_stage();
        let castle_family_counts = |economy: &Level2EconomyState| {
            (
                economy
                    .castle_children()
                    .iter()
                    .filter(|child| child.active && child.model == 79)
                    .count(),
                economy
                    .castle_children()
                    .iter()
                    .filter(|child| child.active && child.model == 3 && child.action == 9)
                    .count(),
                economy
                    .castle_children()
                    .iter()
                    .filter(|child| child.active && child.source_class == 5 && child.model == 15)
                    .count(),
            )
        };
        let baseline_hash = chain.campaign_state_hash();
        let baseline_terrain_hash = chain.terrain_hash();
        let baseline_free_count = chain.economy().free_slots.len();
        let baseline_family_counts = castle_family_counts(chain.economy());
        let starting_holder_slots =
            chain.shared_spells().local_player.enabled_spell_holders[..4].to_vec();
        assert_eq!(
            starting_holder_slots,
            vec![115, 116, 117, 118],
            "qualified direct-start materialization must retain source holder order"
        );
        assert!(
            starting_holder_slots
                .iter()
                .all(|slot| !chain.economy().free_slots.contains(slot)),
            "shared-spell holder slots are baseline-owned and cannot re-enter the economy free stack"
        );
        let allocator_baseline = chain.economy().free_slots.clone();
        let chain_castle_slot = chain
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("controller-chain Castle");
        assert_eq!(
            Some(chain_castle_slot),
            allocator_baseline.last().copied(),
            "Castle allocation must consume the lowest post-holder source slot"
        );
        assert!(
            chain
                .economy_mut()
                .start_castle_construction(chain_castle_slot)
        );
        assert!(chain.economy_mut().step_castle_action(chain_castle_slot));
        {
            let economy = chain.economy_mut();
            assert!(economy.set_castle_controller_coefficient(2, 0x2a));
            let castle = economy.castle.as_mut().expect("stage-two Castle");
            castle.action = 5;
            castle.build_state = 3;
            castle.level = 1;
            for child in &mut economy.castle_children {
                if child.parent_entity_index == chain_castle_slot && child.model == 42 {
                    child.active = false;
                }
            }
        }
        assert!(chain.economy_mut().step_castle_action(chain_castle_slot));
        let old_controller_slots = {
            let castle = chain.economy().castle().expect("stage-two Castle");
            let count = usize::from(castle.controller_chain_count);
            assert_eq!(count, 4);
            castle.controller_chain_slots[..count].to_vec()
        };
        let old_controller_generations = old_controller_slots
            .iter()
            .map(|slot| (*slot, chain.economy().slot_generation(*slot)))
            .collect::<BTreeMap<_, _>>();
        assert!(
            old_controller_slots
                .iter()
                .all(|slot| allocator_baseline.contains(slot)
                    && !starting_holder_slots.contains(slot)),
            "controller rows must come from the economy baseline, never from holder slots"
        );
        assert_eq!(
            chain
                .economy()
                .castle_children()
                .iter()
                .filter(|child| {
                    child.active
                        && child.model == 79
                        && child.source_word_4a == 2
                        && old_controller_slots.contains(&child.source_entity_index)
                })
                .count(),
            4
        );
        chain.select_level2_campaign_entry_stage();
        {
            let economy = chain.economy_mut();
            let castle = economy.castle.as_mut().expect("stage-two Castle");
            // Keep the real stage-two controller chain while selecting the
            // source maintenance tail that owns model-15 rows. This is only
            // branch setup; the production maintenance helper creates the
            // rows here, while teardown alone uses the ordinary Level 2
            // source cursor, which also observes terminal auxiliary cleanup.
            castle.level = 3;
            castle.action = 4;
            castle.timer = 0;
            castle.phase &= !1;
            castle.auxiliary_cooldown = 0;
            castle.mana = castle.mana_capacity + 1_000;
        }
        chain
            .economy_mut()
            .maintain_castle_defenders(chain_castle_slot);
        let (defender_slots, auxiliary_slots) = {
            let castle = chain.economy().castle().expect("stage-two Castle");
            let defender_count = usize::from(castle.defender_count);
            let auxiliary_count = usize::from(castle.auxiliary_count);
            (
                castle.defender_slots[..defender_count]
                    .iter()
                    .copied()
                    .filter(|slot| *slot != 0)
                    .collect::<Vec<_>>(),
                castle.auxiliary_slots[..auxiliary_count]
                    .iter()
                    .copied()
                    .filter(|slot| *slot != 0)
                    .collect::<Vec<_>>(),
            )
        };
        assert!(!defender_slots.is_empty());
        assert!(!auxiliary_slots.is_empty());
        let auxiliary_generations = auxiliary_slots
            .iter()
            .map(|slot| (*slot, chain.economy().slot_generation(*slot)))
            .collect::<BTreeMap<_, _>>();
        for slot in defender_slots.iter().chain(auxiliary_slots.iter()) {
            assert!(chain.economy().map_chains().is_linked(*slot));
        }
        {
            let castle = chain
                .economy_mut()
                .castle
                .as_mut()
                .expect("stage-two Castle");
            castle.action = 6;
            castle.level = 2;
            castle.life = -1;
        }
        chain.step_outer_turn(PlayerCommand::default(), 0);
        let castle = chain.economy().castle().expect("downgraded Castle");
        assert_eq!(castle.level, 1);
        assert!(
            chain
                .economy()
                .castle_children
                .iter()
                .all(|child| { !(child.active && child.model == 79 && child.source_word_4a == 2) })
        );
        assert!(defender_slots.iter().all(|slot| {
            chain
                .economy()
                .castle_children()
                .iter()
                .any(|child| child.active && child.source_entity_index == *slot)
                && chain.economy().map_chains().is_linked(*slot)
        }));
        for slot in &auxiliary_slots {
            assert!(chain.dispatch_events().iter().any(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::DynamicArena {
                        source_slot,
                        kind: Level2DynamicArenaKind::CastleAuxiliary,
                        ..
                    } if *source_slot == *slot
                )
            }));
            assert!(
                chain
                    .economy()
                    .castle_children()
                    .iter()
                    .all(|child| child.source_entity_index != *slot || !child.active)
            );
            let live_row = chain
                .economy()
                .live_arena_cursor_rows()
                .into_iter()
                .find(|row| row.source_slot == *slot);
            match live_row {
                None => {
                    assert!(!chain.economy().map_chains().is_linked(*slot));
                    assert!(chain.economy().free_slots.contains(slot));
                }
                Some(row) => {
                    // The source reclaim is immediately eligible for a
                    // later constructor in this same outer pass. A reused
                    // slot must carry a new generation and a coherent live
                    // map row; it is not an orphaned auxiliary entity.
                    assert_ne!(
                        row.generation,
                        auxiliary_generations
                            .get(slot)
                            .copied()
                            .expect("auxiliary generation")
                    );
                    assert_eq!(row.kind, Level2DynamicArenaKind::ManaAttraction);
                    assert!(!chain.economy().free_slots.contains(slot));
                    assert!(chain.economy().map_chains().is_linked(*slot));
                }
            }
        }
        let live_rows = chain.economy().live_arena_cursor_rows();
        let old_slot_states = old_controller_slots
            .iter()
            .map(|slot| {
                (
                    *slot,
                    old_controller_generations
                        .get(slot)
                        .copied()
                        .expect("old controller generation"),
                    live_rows
                        .iter()
                        .find(|row| row.source_slot == *slot)
                        .map(|row| (row.generation, row.kind)),
                    chain.economy().free_slots.contains(slot),
                    chain.economy().map_chains().is_linked(*slot),
                )
            })
            .collect::<Vec<_>>();
        assert!(
            old_slot_states
                .iter()
                .all(|(_, old_generation, live, free, linked)| match live {
                    None => *free && !*linked,
                    Some((generation, kind)) => {
                        *generation != *old_generation
                            && !*free
                            && (*linked || matches!(kind, Level2DynamicArenaKind::CastleChild))
                    }
                }),
            "retired controller slots must be free or source-reused with a new generation: {old_slot_states:?}"
        );
        assert_ne!(chain.campaign_state_hash(), baseline_hash);
        assert_ne!(chain.economy().free_slots.len(), baseline_free_count);
        assert_ne!(
            castle_family_counts(chain.economy()),
            baseline_family_counts
        );
        chain.reset();
        assert_eq!(chain.campaign_state_hash(), baseline_hash);
        assert_eq!(chain.terrain_hash(), baseline_terrain_hash);
        assert_eq!(chain.economy().free_slots.len(), baseline_free_count);
        assert_eq!(
            castle_family_counts(chain.economy()),
            baseline_family_counts
        );

        // Exhaust every ordinary arena row while retaining two flagged rows.
        // All constructors use the global `NewEvent_4A050` chokepoint; the
        // generic death-burst producer is the discriminating consumer here.
        chain.select_level2_campaign_entry_stage();
        let retry_castle_slot = chain
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("retry allocator Castle");
        {
            let economy = chain.economy_mut();
            let castle = economy.castle.as_mut().expect("retry allocator Castle");
            castle.level = 3;
            castle.action = 4;
            castle.phase &= !1;
            castle.auxiliary_cooldown = 0;
        }
        chain
            .economy_mut()
            .maintain_castle_defenders(retry_castle_slot);
        let retry_slot = chain
            .economy()
            .castle()
            .expect("retry allocator Castle")
            .auxiliary_slots[0];
        assert_ne!(retry_slot, 0);
        assert!(chain.economy().castle_children().iter().any(|child| {
            child.active
                && child.source_entity_index == retry_slot
                && child.source_class == 5
                && child.model == 15
                && child.control_flags & SOURCE_REPLACEABLE_FLAG != 0
                && child.parent_entity_index == 0
        }));
        let preexisting_death_burst_slot = chain
            .economy_mut()
            .spawn_death_burst_from_death_with_id(retry_castle_slot, 111, Position::default())
            .expect("second flagged retry row");
        assert!(retry_slot < preexisting_death_burst_slot);
        assert_eq!(
            chain
                .economy()
                .castle_children()
                .iter()
                .filter(|child| {
                    child.active
                        && child.source_class == 5
                        && child.model == 15
                        && child.control_flags & SOURCE_REPLACEABLE_FLAG != 0
                })
                .map(|child| child.source_entity_index)
                .chain(
                    chain
                        .economy()
                        .death_bursts()
                        .iter()
                        .filter(|burst| {
                            burst.active && burst.control_flags & SOURCE_REPLACEABLE_FLAG != 0
                        })
                        .map(|burst| burst.source_entity_index),
                )
                .collect::<Vec<_>>(),
            vec![retry_slot, preexisting_death_burst_slot]
        );
        let ordinary_slots = chain.economy().free_slots.len();
        let filled_rows = {
            let economy = chain.economy_mut();
            for _ in 0..ordinary_slots {
                assert!(
                    economy
                        .spawn_mana_sphere_from_death(52, Position::default(), 0, 0x9876, 1, 111)
                        .is_some()
                );
            }
            ordinary_slots
        };
        assert_eq!(filled_rows, ordinary_slots);
        assert_eq!(filled_rows + 4, baseline_free_count);
        assert!(chain.economy().free_slots.is_empty());
        let death_burst_count = chain.economy().death_bursts().len();
        let retry_generation_before = chain.economy().slot_generation(retry_slot);
        let retry_global_rng = chain.economy().global_rng;
        let consumed_slot = chain
            .economy_mut()
            .spawn_death_burst_from_death_with_id(retry_castle_slot, 111, Position::default())
            .expect("global retry allocator consumes Castle model-15 row");
        assert_eq!(consumed_slot, retry_slot);
        assert_eq!(chain.economy().death_bursts().len(), death_burst_count + 1);
        let replacement_burst = chain
            .economy()
            .death_bursts()
            .iter()
            .find(|burst| burst.active && burst.source_entity_index == retry_slot)
            .expect("replacement death burst");
        assert_eq!(
            replacement_burst.entity_rng,
            retry_slot.wrapping_add(retry_global_rng as u16)
        );
        assert_ne!(
            chain.economy().slot_generation(retry_slot),
            retry_generation_before
        );
        assert!(!chain.economy().slot_rng.contains_key(&retry_slot));
        assert!(!chain.economy().castle_children().iter().any(|child| {
            child.active
                && child.source_entity_index == retry_slot
                && child.source_class == 5
                && child.model == 15
        }));
        assert_eq!(
            chain
                .economy()
                .castle()
                .expect("retry allocator Castle")
                .auxiliary_slots[0],
            retry_slot
        );
        assert!(chain.economy().map_chains().is_linked(retry_slot));
        assert!(chain.economy().free_slots.is_empty());

        // `sub_5FF50` owns the stale Castle relation cleanup; allocation must
        // leave it visible until that production maintenance walk runs.
        chain
            .economy_mut()
            .maintain_castle_defenders(retry_castle_slot);
        assert_eq!(
            chain
                .economy()
                .castle()
                .expect("retry allocator Castle")
                .auxiliary_slots[0],
            0
        );

        chain.reset();
        assert_eq!(chain.campaign_state_hash(), baseline_hash);
        assert_eq!(chain.terrain_hash(), baseline_terrain_hash);
        assert_eq!(chain.economy().free_slots.len(), baseline_free_count);
        assert_eq!(
            castle_family_counts(chain.economy()),
            baseline_family_counts
        );

        // Source `sub_49F90` orders every flagged live row by arena slot, not
        // by producer family. A lower authored model-6 row must therefore
        // beat the later Castle model-15 auxiliary when the generic death
        // burst constructor reaches the shared NewEvent fallback.
        chain.select_level2_campaign_entry_stage();
        let lower_model6 = chain
            .economy_mut()
            .allocate_authored_model6(361, Position::default())
            .expect("lower flagged authored model-6 row");
        let lower_model6_slot = lower_model6.source_entity_index;
        let lower_model6_rng = chain
            .economy()
            .source_rng
            .get(&lower_model6_slot)
            .copied()
            .expect("authored model-6 source RNG");
        assert_eq!(
            chain.economy().source_rng.get(&lower_model6_slot),
            Some(&lower_model6_rng)
        );
        let lower_model6_generation = chain.economy().slot_generation(lower_model6_slot);
        let lower_castle_slot = chain
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("lower-slot discriminator Castle");
        {
            let economy = chain.economy_mut();
            let castle = economy
                .castle
                .as_mut()
                .expect("lower-slot discriminator Castle");
            castle.level = 3;
            castle.action = 4;
            castle.phase &= !1;
            castle.auxiliary_cooldown = 0;
        }
        chain
            .economy_mut()
            .maintain_castle_defenders(lower_castle_slot);
        let lower_case_auxiliary_slot = chain
            .economy()
            .castle()
            .expect("lower-slot discriminator Castle")
            .auxiliary_slots[0];
        assert_ne!(lower_case_auxiliary_slot, 0);
        assert!(lower_model6_slot < lower_case_auxiliary_slot);
        assert_eq!(
            chain
                .economy()
                .authored_model6
                .iter()
                .filter(|source| {
                    source.active
                        && source.source_entity_index == lower_model6_slot
                        && source.control_flags & SOURCE_REPLACEABLE_FLAG != 0
                })
                .count(),
            1
        );
        assert!(chain.economy().castle_children().iter().any(|child| {
            child.active
                && child.source_entity_index == lower_case_auxiliary_slot
                && child.source_class == 5
                && child.model == 15
                && child.control_flags & SOURCE_REPLACEABLE_FLAG != 0
        }));
        chain.economy_mut().free_slots.clear();
        let replacement_global_rng = chain.economy().global_rng;
        let consumed_slot = chain
            .economy_mut()
            .spawn_death_burst_from_death_with_id(lower_castle_slot, 111, Position::default())
            .expect("global fallback consumes lower authored model-6 row");
        assert_eq!(consumed_slot, lower_model6_slot);
        assert_eq!(
            chain
                .economy()
                .authored_model6
                .iter()
                .filter(|source| source.source_entity_index == lower_model6_slot)
                .count(),
            0,
            "typed authored model-6 row was evicted in place"
        );
        assert!(chain.economy().castle_children().iter().any(|child| {
            child.active
                && child.source_entity_index == lower_case_auxiliary_slot
                && child.source_class == 5
                && child.model == 15
        }));
        let replacement_burst = chain
            .economy()
            .death_bursts()
            .iter()
            .find(|burst| burst.active && burst.source_entity_index == lower_model6_slot)
            .expect("replacement death burst at authored model-6 slot");
        assert_eq!(
            replacement_burst.entity_rng,
            lower_model6_slot.wrapping_add(replacement_global_rng as u16)
        );
        assert_eq!(
            chain.economy().source_rng.get(&lower_model6_slot),
            None,
            "eviction clears the old authored model-6 source RNG"
        );
        assert_eq!(
            chain.economy().slot_rng.get(&lower_model6_slot),
            None,
            "eviction clears the old slot RNG"
        );
        assert_ne!(
            chain.economy().slot_generation(lower_model6_slot),
            lower_model6_generation
        );
        assert!(chain.economy().map_chains().is_linked(lower_model6_slot));
        assert_eq!(
            chain
                .economy()
                .castle()
                .expect("lower-slot discriminator Castle")
                .auxiliary_slots[0],
            lower_case_auxiliary_slot,
            "Castle auxiliary relation remains until sub_5FF50 maintenance"
        );
        chain
            .economy_mut()
            .maintain_castle_defenders(lower_castle_slot);
        assert_eq!(
            chain
                .economy()
                .castle()
                .expect("lower-slot discriminator Castle")
                .auxiliary_slots[0],
            lower_case_auxiliary_slot,
            "maintenance preserves the still-live higher Castle auxiliary"
        );
        chain.reset();
        assert_eq!(chain.campaign_state_hash(), baseline_hash);
        assert_eq!(chain.terrain_hash(), baseline_terrain_hash);
        assert_eq!(chain.economy().free_slots.len(), baseline_free_count);
        assert_eq!(
            castle_family_counts(chain.economy()),
            baseline_family_counts
        );
    }

    #[test]
    fn level_two_mana_sphere_vertical_velocity_uses_signed_source_adjustment() {
        assert_eq!(level2_mana_sphere_vertical_velocity(0), 127);
        assert_eq!(level2_mana_sphere_vertical_velocity(1_024), 0);
        assert_eq!(level2_mana_sphere_vertical_velocity(2_048), -128);
    }

    #[test]
    fn level_two_castle_spawn_carrier_accepts_source_launch_payload_and_hashes_relation() {
        let package = level_two_fixture_path();
        let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
            .expect("qualified Level 2 gameplay component");
        let mut session = inspection
            .level2_reset_session()
            .expect("Level 2 Castle source-payload session");
        session.select_level2_campaign_entry_stage();
        let baseline_hash = session.campaign_state_hash();
        let payload = Level2CastleProjectileLaunchPayload {
            position: Position {
                x: 8_192,
                y: 8_448,
                z: 256,
            },
            yaw: 1_792,
            pitch: -321,
            source_id_entity_index: 111,
            source_subspell_index: 2,
            source_mana: 2_400,
            source_a4_byte_154: 7,
        };
        let flyer_slot = session
            .economy_mut()
            .spawn_castle_projectile_with_payload(payload, 0, 37)
            .expect("source-payload Castle allocation");
        let flyer = session
            .economy()
            .castle_projectiles()
            .iter()
            .find(|projectile| projectile.source_entity_index == flyer_slot)
            .copied()
            .expect("source-payload Castle carrier");
        assert_eq!(flyer.position, payload.position);
        assert_eq!(flyer.yaw, payload.yaw);
        assert_eq!(flyer.pitch, payload.pitch);
        assert_eq!(flyer.owner_entity_index, payload.source_id_entity_index);
        assert_eq!(flyer.actual_speed, 384_i16.wrapping_add(37));
        assert_eq!(flyer.minimum_speed, 384);
        assert_eq!(
            session.economy().source_owner(flyer_slot),
            payload.source_id_entity_index,
            "the source id relation is carried by the hashed economy owner map"
        );
        assert!(session.economy().map_chains().is_linked(flyer_slot));
        assert_ne!(session.campaign_state_hash(), baseline_hash);

        session
            .economy_mut()
            .disable_castle_projectile(flyer_slot, true);
        session
            .economy_mut()
            .reclaim_hidden_castle_rows_at_source_boundary();
        assert_eq!(session.economy().source_owner(flyer_slot), 0);
        assert!(!session.economy().map_chains().is_linked(flyer_slot));
    }

    #[test]
    fn level_two_castle_flyer_lands_on_post_move_terrain_intercept() {
        let package = level_two_fixture_path();
        let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
            .expect("qualified Level 2 gameplay component");
        let mut session = inspection
            .level2_reset_session()
            .expect("Level 2 terrain-intercept session");
        session.select_level2_campaign_entry_stage();
        session.step_outer_turn(
            PlayerCommand {
                opcode: 6,
                argument_1: 2,
                flags: 0x40,
                ..PlayerCommand::default()
            },
            0,
        );
        let flyer = session
            .economy()
            .castle_projectiles()
            .iter()
            .find(|projectile| projectile.active)
            .copied()
            .expect("new-Castle flyer remains live after its same-pass gate-only callback");
        let ground = session.terrain.ground_height(flyer.position);
        let blocker_index = session
            .entities
            .iter()
            .position(|entity| !entity.active && entity.index != 0)
            .expect("free source row for prior-landing gate blocker");
        let blocker = &mut session.entities[blocker_index];
        blocker.active = true;
        blocker.life = -1;
        blocker.class = 10;
        blocker.model = 45;
        blocker.record[88..90].copy_from_slice(&0_i16.to_le_bytes());
        blocker.record[90..92].copy_from_slice(&0_i16.to_le_bytes());
        blocker.set_position(flyer.position);
        {
            let projectile = session
                .economy_mut()
                .castle_projectiles
                .iter_mut()
                .find(|projectile| projectile.source_entity_index == flyer.source_entity_index)
                .expect("live flyer row");
            projectile.position.z = ground + 1;
            projectile.axis_position = Position {
                x: projectile.position.x,
                y: projectile.position.y,
                z: ground - 1,
            };
            projectile.life = projectile.max_life;
        }
        let before_landing_yaw = session
            .economy()
            .castle_projectiles()
            .iter()
            .find(|projectile| projectile.source_entity_index == flyer.source_entity_index)
            .expect("terrain-intercept flyer")
            .yaw;
        let entities = session.entities.clone();
        assert!(
            !session
                .economy_mut()
                .step_castle_projectile_slot(flyer.source_entity_index, &entities),
            "post-move ground interception lands the flyer before life expiry"
        );
        let root = session
            .economy()
            .castle()
            .expect("terrain landing allocates the selected class-3/model-2 root");
        assert_eq!((root.source_class, root.source_model), (3, 2));
        assert!(
            session
                .economy()
                .castle_projectiles()
                .iter()
                .all(|projectile| !projectile.active),
            "terrain landing consumes the flyer after 4A190 succeeds"
        );
        let consumed_flyer = session
            .economy()
            .castle_projectiles()
            .iter()
            .find(|projectile| projectile.source_entity_index == flyer.source_entity_index)
            .expect("consumed terrain-intercept flyer");
        assert_eq!(
            consumed_flyer.yaw, before_landing_yaw,
            "prior terrain landing does not enter the later 11CB0 bounce branch"
        );
    }

    #[test]
    fn level_two_castle_flyer_later_site_gate_bounces_and_retries_root() {
        let package = level_two_fixture_path();
        let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
            .expect("qualified Level 2 gameplay component");
        let mut session = inspection
            .level2_reset_session()
            .expect("Level 2 later-gate session");
        session.economy_mut().terrain.planes.angles.fill(0);
        let initial = Position {
            x: 8_192,
            y: 8_192,
            z: 10_000,
        };
        let flyer_slot = session
            .economy_mut()
            .spawn_castle_projectile(initial, 111, 0, 0, 0, 0)
            .expect("new-Castle flyer");
        assert!(
            session
                .economy_mut()
                .step_castle_projectile_slot(flyer_slot, &[])
        );
        {
            let projectile = session
                .economy_mut()
                .castle_projectiles
                .iter_mut()
                .find(|projectile| projectile.source_entity_index == flyer_slot)
                .expect("gate-initialized flyer");
            projectile.position = initial;
            projectile.axis_position = Position {
                x: 8_400,
                y: initial.y,
                z: initial.z,
            };
            projectile.yaw = 512;
            projectile.life = projectile.max_life;
        }
        let blocker_index = session
            .entities
            .iter()
            .position(|entity| !entity.active && entity.index != 0)
            .expect("free source row for later gate blocker");
        let blocker = &mut session.entities[blocker_index];
        blocker.active = true;
        blocker.life = -1;
        blocker.class = 10;
        blocker.model = 45;
        blocker.record[88..90].copy_from_slice(&0_i16.to_le_bytes());
        blocker.record[90..92].copy_from_slice(&0_i16.to_le_bytes());
        blocker.set_position(initial);
        let entities = session.entities.clone();
        let free_slots = session.economy().free_slots.clone();
        session.economy_mut().free_slots.clear();
        let before_failed_retry = session
            .economy()
            .castle_projectiles()
            .iter()
            .find(|projectile| projectile.source_entity_index == flyer_slot)
            .copied()
            .expect("live flyer before pure gate failure");
        assert!(before_failed_retry.position.z > session.terrain.ground_height(initial));
        assert_eq!(before_failed_retry.life, before_failed_retry.max_life);
        assert!(
            session
                .economy_mut()
                .step_castle_projectile_slot(flyer_slot, &entities)
        );
        let bounced = session
            .economy()
            .castle_projectiles()
            .iter()
            .find(|projectile| projectile.source_entity_index == flyer_slot)
            .copied()
            .expect("allocation failure leaves bounced flyer live");
        assert!(bounced.active);
        assert_ne!(bounced.position, before_failed_retry.position);
        assert_eq!(bounced.yaw, 1_536);
        assert_eq!(bounced.life, bounced.max_life - 1);
        assert!(session.economy().castle().is_none());

        session.economy_mut().free_slots = free_slots;
        assert!(
            !session
                .economy_mut()
                .step_castle_projectile_slot(flyer_slot, &entities)
        );
        let root = session
            .economy()
            .castle()
            .expect("later gate retry allocates the Castle root");
        assert_eq!((root.source_class, root.source_model), (3, 2));
        assert_ne!(root.position, initial);
        assert!(
            session
                .economy()
                .castle_projectiles()
                .iter()
                .all(|projectile| !projectile.active)
        );
    }

    #[test]
    fn level_two_castle_site_gate_scans_live_castles_and_model45_rows() {
        let package = level_two_fixture_path();
        let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
            .expect("qualified Level 2 gameplay component");
        let mut session = inspection
            .level2_reset_session()
            .expect("Level 2 site-gate session");
        session.select_level2_campaign_entry_stage();
        let request = PlayerCommand {
            opcode: 6,
            argument_1: 2,
            flags: 0x40,
            ..PlayerCommand::default()
        };
        for (class, model) in [(3_u8, 2_u8), (10_u8, 45_u8)] {
            session.reset();
            session.select_level2_campaign_entry_stage();
            let blocker_index = session
                .entities
                .iter()
                .position(|entity| !entity.active && entity.index != 0)
                .expect("free source row for site-gate blocker");
            let blocker = &mut session.entities[blocker_index];
            blocker.active = true;
            blocker.life = if class == 10 && model == 45 { -1 } else { 8 };
            blocker.class = class;
            blocker.model = model;
            blocker.record[88..90].copy_from_slice(&0_i16.to_le_bytes());
            blocker.record[90..92].copy_from_slice(&0_i16.to_le_bytes());
            blocker.set_position(session.state.position);

            // The source cursor visits the newly allocated flyer after the
            // holder in this same request pass. Install the live blocker
            // before that request so the first 11CB0 gate rejects the flyer;
            // inserting it after the request would probe the later landing
            // branch and incorrectly permit a Castle root.
            session.step_outer_turn(request, 0);
            assert!(session.economy().castle().is_none());
            let flyer = session
                .economy()
                .castle_projectiles()
                .iter()
                .find(|projectile| projectile.site_rejected && !projectile.active)
                .copied()
                .expect("same-pass site gate hides the blocked Castle flyer");
            assert_eq!((flyer.source_class, flyer.source_model), (9, 10));
            let flyer_generation = session.economy().slot_generation(flyer.source_entity_index);
            assert!(flyer.drawing_disabled);
            assert!(
                session
                    .economy()
                    .map_chains()
                    .is_linked(flyer.source_entity_index)
            );
            assert!(
                !session
                    .economy()
                    .free_slots
                    .contains(&flyer.source_entity_index)
            );
            assert!(!session.dispatch_events().iter().any(|event| matches!(
                event,
                Level2DispatchEvent::DynamicArena {
                    kind: Level2DynamicArenaKind::CastleProjectile,
                    ..
                }
            )));

            // DisableEntityDrawing keeps the row linked and reserved until
            // the next generic hidden-row prepass, where the source frees it.
            session.step_outer_turn(PlayerCommand::default(), 0);
            assert!(
                !session
                    .economy()
                    .castle_projectiles()
                    .iter()
                    .any(|projectile| projectile.source_entity_index == flyer.source_entity_index)
            );
            assert!(
                session
                    .economy()
                    .free_slots
                    .contains(&flyer.source_entity_index)
            );
            assert!(
                !session
                    .economy()
                    .map_chains()
                    .is_linked(flyer.source_entity_index)
            );
            assert_eq!(
                session.economy().slot_generation(flyer.source_entity_index),
                flyer_generation
            );
        }
    }

    #[test]
    fn level_two_economy_sound_receipts_pin_source_owner_channel_and_negatives() {
        let mut session = level_two_possession_lifecycle_session();

        // AUD-L2-036: the qualified Quickfair callback latches sound before
        // its three shape-cell RNG branches. Drive the real row-214 ->
        // group-20 source chain; no allocator or narrative helper is used.
        let authored_features = session.mission.authored_features().to_vec();
        let feature_position = |index: u16| {
            authored_features
                .iter()
                .copied()
                .find(|feature| feature.index == index)
                .expect("qualified Quickfair source-chain row")
                .world_position(0)
        };
        session.state.position = feature_position(11);
        session.runtime.objectives[0].stage_selectors[1] = 2;
        session.runtime.objectives[0].stage_selectors[2] = 2;
        session.step_outer_turn(PlayerCommand::default(), 0);
        session.runtime.objectives[0].stage_selectors[3] = 2;
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            session
                .economy()
                .authored_switch_producers()
                .iter()
                .any(|source| source.authored_index == 214)
        );
        session.state.position = feature_position(214);
        let expected_group20 = [
            216_u16, 217, 218, 219, 220, 221, 222, 223, 232, 233, 234, 235,
        ];
        let mut activated = false;
        for _ in 0..8 {
            session.step_outer_turn(PlayerCommand::default(), 0);
            if session.dispatch_events().iter().any(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::AuthoredGroupActivated {
                        group: 20,
                        authored_indices,
                        ..
                    } if authored_indices.as_ref() == expected_group20
                )
            }) {
                activated = true;
                break;
            }
        }
        assert!(activated, "row 214 must activate group 20 in production");
        let quickfair = session
            .economy()
            .authored_model1()
            .iter()
            .find(|source| source.authored_index == 216)
            .copied()
            .expect("production group-20 Quickfair source");
        assert_eq!(
            session
                .dispatch_events()
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        Level2DispatchEvent::Economy {
                            source_slot,
                            event: Level2EconomyEvent::PreparedSound {
                                source_entity_index,
                                entity_index,
                                player_index: -1,
                                sound_id: 3,
                            },
                            ..
                        } if *source_slot == quickfair.source_entity_index
                            && *source_entity_index == quickfair.source_entity_index
                            && *entity_index == quickfair.source_entity_index
                    )
                })
                .count(),
            1,
            "the production Quickfair callback emits one sound-3 receipt"
        );

        // A following callback does not duplicate the sound latch.
        session.economy_mut().begin_turn();
        assert!(
            session
                .economy_mut()
                .step_authored_model1_slot(quickfair.source_entity_index)
        );
        assert!(session.economy().events().is_empty());

        // The negative life guard precedes the sound latch and all Quickfair
        // source draws.
        session.economy_mut().begin_turn();
        let before = session
            .economy()
            .authored_model1()
            .iter()
            .find(|source| source.source_entity_index == quickfair.source_entity_index)
            .copied()
            .expect("Quickfair remains live for negative branch");
        session
            .economy_mut()
            .authored_model1
            .iter_mut()
            .find(|source| source.source_entity_index == quickfair.source_entity_index)
            .expect("Quickfair negative branch source")
            .life = -1;
        assert!(
            session
                .economy_mut()
                .step_authored_model1_slot(quickfair.source_entity_index)
        );
        let after = session
            .economy()
            .authored_model1()
            .iter()
            .find(|source| source.source_entity_index == quickfair.source_entity_index)
            .copied()
            .expect("Quickfair remains inspectable after negative branch");
        assert!(session.economy().events().is_empty());
        assert_eq!(after.entity_rng, before.entity_rng);

        // AUD-L2-056/057: both accepted possession branches carry the house
        // source as the handler slot and v7/v8 as the exact owner argument.
        session.reset();
        let blocked_house = session
            .ordinary()
            .houses()
            .iter()
            .copied()
            .find(|house| house.parameter.possession_blocked())
            .expect("pinned BLDGPRM possession negative");
        let blocked_owner = session.state.entity_index;
        assert!(
            session
                .ordinary_mut()
                .submit_house_possession(blocked_house.entity_index, blocked_owner, 1)
                .is_err()
        );
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(!session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Economy {
                    event: Level2EconomyEvent::PreparedSound { sound_id: 4, .. },
                    ..
                }
            )
        }));
        session.reset();
        let house = session
            .ordinary()
            .houses()
            .iter()
            .copied()
            .find(|house| house.parameter.produces_villagers())
            .expect("qualified Level 2 possession house");
        let owner = session.state.entity_index;
        session
            .ordinary_mut()
            .submit_house_possession(house.entity_index, owner, 1)
            .expect("payload-one house possession mailbox");
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Economy {
                    source_slot,
                    event: Level2EconomyEvent::PreparedSound {
                        source_entity_index,
                        entity_index,
                        player_index: -1,
                        sound_id: 4,
                    },
                    ..
                } if *source_slot == house.entity_index
                    && *source_entity_index == house.entity_index
                    && *entity_index == owner
            )
        }));
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(!session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Economy {
                    source_slot,
                    event: Level2EconomyEvent::PreparedSound { sound_id: 4, .. },
                    ..
                } if *source_slot == house.entity_index
            )
        }));
        session.reset();
        session
            .ordinary_mut()
            .submit_house_possession(house.entity_index, owner, 0)
            .expect("payload-zero house possession mailbox");
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Economy {
                    source_slot,
                    event: Level2EconomyEvent::PreparedSound {
                        source_entity_index,
                        entity_index,
                        player_index: -1,
                        sound_id: 4,
                    },
                    ..
                } if *source_slot == house.entity_index
                    && *source_entity_index == house.entity_index
                    && *entity_index == owner
            )
        }));
        session.reset();
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(!session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Economy {
                    event: Level2EconomyEvent::PreparedSound { sound_id: 4, .. },
                    ..
                }
            )
        }));

        // AUD-L2-080: InitSwitchChainZaxisAndSound resolves the first
        // overlapping model-0 owner (the local player here), then emits
        // before the group-10 model-76 allocation is announced.
        session.reset();
        let switch = session
            .level2_group10_switch25
            .expect("qualified imported model-0 switch 25");
        session.state.position = switch.position;
        session
            .level2_group10_switch25
            .as_mut()
            .expect("switch 25 remains live")
            .phase = 0;
        let owner = session.state.entity_index;
        session.step_outer_turn(PlayerCommand::default(), 0);
        let sound_index = session
            .dispatch_events()
            .iter()
            .position(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::Economy {
                        pass: 0,
                        source_slot,
                        event: Level2EconomyEvent::PreparedSound {
                            source_entity_index,
                            entity_index,
                            player_index: -1,
                            sound_id: 41,
                        },
                        ..
                    } if *source_slot == switch.source_slot
                        && *source_entity_index == switch.source_slot
                        && *entity_index == owner
                )
            })
            .expect("source pass emits switch sound 41");
        let group_index = session
            .dispatch_events()
            .iter()
            .position(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::AuthoredGroupActivated {
                        source_slot,
                        group: 10,
                        ..
                    } if *source_slot == switch.source_slot
                )
            })
            .expect("source pass announces switch group 10");
        assert!(sound_index < group_index, "sound precedes group activation");
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated {
                    source_slot,
                    group: 10,
                    ..
                } if *source_slot == switch.source_slot
            )
        }));
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(!session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Economy {
                    source_slot,
                    event: Level2EconomyEvent::PreparedSound { sound_id: 41, .. },
                    ..
                } if *source_slot == switch.source_slot
            )
        }));

        // Wrong overlap and source-arena exhaustion consume no forbidden
        // sound receipt and leave the switch available for a later source
        // turn.
        session.reset();
        let switch = session
            .level2_group10_switch25
            .expect("switch 25 after reset");
        session.state.position = Position {
            x: switch.position.x.wrapping_add(0x4000),
            ..switch.position
        };
        session
            .level2_group10_switch25
            .as_mut()
            .expect("switch 25 non-overlap state")
            .phase = 0;
        session.step_level2_group10_switch25(0);
        assert!(session.dispatch_events().is_empty());
        assert!(session.level2_group10_switch25.is_some());

        session.reset();
        let switch = session
            .level2_group10_switch25
            .expect("switch 25 exhaustion state");
        session.state.position = switch.position;
        session
            .level2_group10_switch25
            .as_mut()
            .expect("switch 25 exhaustion phase")
            .phase = 0;
        session.economy_mut().free_slots.clear();
        session.step_level2_group10_switch25(0);
        assert!(!session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Economy {
                    event: Level2EconomyEvent::PreparedSound { sound_id: 41, .. },
                    ..
                }
            )
        }));
    }
}
