use super::*;

impl Level2EconomyState {
    #[must_use]
    pub fn source_owner(&self, source_entity_index: u16) -> u16 {
        self.source_owner
            .get(&source_entity_index)
            .copied()
            .unwrap_or_default()
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
    /// class-5/model-15 or class-10/model-41/42/79 rows: it sets the drawing
    /// disable bit and immediately calls `sub_57F20`, which unlinks the map
    /// row and returns the entity to the `NewEvent_4A050` free stack. Runtime
    /// Castle creation (`sub_5FF50`/`sub_613D0`) remains a separate path.
    pub fn applyevents_terminal_reclaim_castle_child(
        &mut self,
        source_slot: u16,
    ) -> Option<Level2CastleChild> {
        let Some(child_index) = self.castle_children.iter().position(|child| {
            child.active
                && child.source_entity_index == source_slot
                && ((child.source_class == 5 && child.model == 15)
                    || (child.source_class == 10 && matches!(child.model, 41 | 42 | 79)))
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

    /// Advance one dynamic Castle child through the no-callback branches of
    /// `ApplyEvents_498A0`.  The pinned source disables drawing for
    /// class-5/model-15 and class-10/model-41/42/79 rows, then immediately
    /// tests the newly written bit and falls through to `sub_57F20` in the
    /// same cursor visit.  Do both writes atomically here so a production
    /// outer turn cannot leave a terminal row as a stale live marker.
    pub fn step_castle_child_terminal_slot(&mut self, source_slot: u16) -> bool {
        let Some(child_index) = self.castle_children.iter().position(|child| {
            child.active
                && child.source_entity_index == source_slot
                && ((child.source_class == 5 && child.model == 15)
                    || (child.source_class == 10 && matches!(child.model, 41 | 42 | 79)))
        }) else {
            return false;
        };
        self.castle_children[child_index].control_flags |= ENTITY_DRAW_DISABLED_BIT;
        self.applyevents_terminal_reclaim_castle_child(source_slot)
            .is_some()
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
        self.build_castle_at(root_position, root_position, owner_entity_index, false)
    }

    /// Build a Castle from the source projectile placement path, retaining
    /// the distinct snapped root position and BUILD-derived construction
    /// anchor. This is the production entry point when the owner BUILD
    /// footprint is available from the validated cache.
    pub fn build_castle_with_footprint(
        &mut self,
        projectile_position: Position,
        owner_entity_index: u16,
        footprint: &CastleBuildFootprint,
    ) -> Option<u16> {
        if footprint.index != 1 {
            return None;
        }
        let (root_position, construction_anchor) =
            level2_castle_root_positions(&self.terrain, footprint, projectile_position);
        let castle_slot =
            self.build_castle_at(root_position, construction_anchor, owner_entity_index, true);
        if castle_slot.is_some() {
            self.castle_footprint = Some(footprint.clone());
            self.castle_stage_footprints
                .insert(footprint.index, footprint.clone());
        }
        castle_slot
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

    pub(super) fn build_castle_at(
        &mut self,
        position: Position,
        anchor_position: Position,
        owner_entity_index: u16,
        source_constructor: bool,
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
        self.events.push(Level2EconomyEvent::CastleBuilt {
            source_entity_index,
            owner_entity_index,
        });
        Some(source_entity_index)
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

    /// Commit the construction child completion callback (`state 2`) after
    /// the model-42 child has finished its own action stream.
    pub fn complete_castle_construction(&mut self, castle_slot: u16) -> bool {
        let construction_slot = {
            let Some(castle) = self
                .castle
                .as_mut()
                .filter(|castle| castle.active && castle.source_entity_index == castle_slot)
            else {
                return false;
            };
            if castle.action != 5 || castle.build_state != 4 || !castle.construction_child_active {
                return false;
            }
            let construction_slot = castle.construction_slot;
            castle.construction_child_active = false;
            castle.child_chain_active[0] = false;
            castle.build_state = 2;
            construction_slot
        };
        if let Some(child) = self
            .castle_children
            .iter_mut()
            .find(|child| child.source_entity_index == construction_slot)
        {
            child.active = false;
            let inactive = *child;
            self.sync_castle_child_map(inactive);
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
    /// preserving source order and the exact final remainder. The source's
    /// subsequent `sub_5FF50` controller walk is represented by the existing
    /// ordered sphere/child vectors; no synthetic child is allocated here.
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
        // The source recomputes the per-sphere share from the number of
        // allocations that can actually be committed, then gives the final
        // source-ordered sphere the exact remainder.
        let each_mana = release / sphere_count;
        let mut remaining = release;
        let mut source_rng = self
            .source_rng
            .get(&castle_slot)
            .copied()
            .unwrap_or_else(|| self.new_event_rng(castle_slot));
        let mut released: i32 = 0;
        for sphere_index in 0..sphere_count {
            let Some(sphere_slot) = self.allocate_slot() else {
                break;
            };
            let sphere_seed = self.new_event_rng(sphere_slot);
            let sphere_rng = level2_next_rng16(sphere_seed);
            let speed_rng = level2_next_rng16(sphere_rng);
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
            let sphere_mana = if sphere_index + 1 == sphere_count {
                remaining
            } else {
                each_mana
            };
            remaining = remaining.saturating_sub(sphere_mana);
            released = released.saturating_add(sphere_mana);
            let sphere = ManaSphere {
                id: u32::from(sphere_slot),
                source_entity_index: sphere_slot,
                entity_rng: speed_rng,
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
                parent_entity_index: snapshot.source_entity_index,
                owner_entity_index: snapshot.owner_entity_index,
                player_entity_index: 0,
                position: Position {
                    z: ground.saturating_add(if selected_level <= 1 { 384 } else { 224 }),
                    ..stage_position
                },
                model: 79,
                action: 0,
                mana: 0,
                max_mana: 0,
                life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                max_life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                speed: LEVEL2_CASTLE_CHILD_DEFAULT_SPEED,
                target_sphere_slot: 0,
                source_byte_3e: stage_slot as u8,
                cave_collision_cooldown: 0,
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

    /// Bounded `sub_5FF50` maintenance after a Castle stage transition.
    /// Existing model-3 action-9 holders are retained in source order, dead
    /// or excess holders are returned to the free arena, and missing slots
    /// are replaced until the `sub_60400` stage count is met. This keeps
    /// allocator fallback deterministic without importing the full combat
    /// targeting arbitration; model-15 auxiliary relation creation and fixed
    /// tail bounds are represented below.
    pub(super) fn maintain_castle_defenders(&mut self, castle_slot: u16) {
        let Some(snapshot) = self
            .castle
            .filter(|castle| castle.active && castle.source_entity_index == castle_slot)
        else {
            return;
        };
        let target = usize::from(level2_castle_defender_count(snapshot.level));
        let mut defenders = self
            .castle_children
            .iter()
            .filter(|child| {
                child.active
                    && child.parent_entity_index == castle_slot
                    && child.model == 3
                    && child.action == 9
            })
            .map(|child| child.source_entity_index)
            .collect::<Vec<_>>();
        for stale_slot in defenders.iter().skip(target).copied().collect::<Vec<_>>() {
            if let Some(child) = self
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == stale_slot)
            {
                child.active = false;
                let inactive = *child;
                self.sync_castle_child_map(inactive);
            }
            self.release_slot(stale_slot, None);
        }
        defenders.truncate(target);
        while defenders.len() < target {
            let Some(slot) = self.allocate_slot() else {
                break;
            };
            let child = Level2CastleChild {
                source_entity_index: slot,
                entity_rng: self.new_event_rng(slot),
                id_entity_index: snapshot.source_id_entity_index,
                source_class: 3,
                parent_entity_index: castle_slot,
                owner_entity_index: snapshot.owner_entity_index,
                player_entity_index: snapshot.source_id_entity_index,
                position: snapshot.position,
                model: 3,
                action: 9,
                mana: 0,
                max_mana: LEVEL2_CASTLE_DEFENDER_MAX_MANA,
                life: LEVEL2_CASTLE_DEFENDER_MAX_LIFE,
                max_life: LEVEL2_CASTLE_DEFENDER_MAX_LIFE,
                speed: LEVEL2_CASTLE_DEFENDER_SPEED,
                target_sphere_slot: 0,
                source_byte_3e: snapshot.phase,
                cave_collision_cooldown: 0,
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
            defenders.push(slot);
        }
        if let Some(castle) = self
            .castle
            .as_mut()
            .filter(|castle| castle.source_entity_index == castle_slot)
        {
            castle.defender_slots = [
                defenders.first().copied().unwrap_or_default(),
                defenders.get(1).copied().unwrap_or_default(),
                defenders.get(2).copied().unwrap_or_default(),
            ];
            castle.defender_count = defenders.len() as u8;
        }

        // `sub_5FF50` resets the owner's aggregate defender mana/max counters
        // and then arbitrates each holder's `word_0x96_150` target. A full
        // Castle first claims the holder; otherwise an eligible action-9
        // holder receives the typed `sub_5F810` model-39 chain selection.
        let mut defender_targets = snapshot.defender_target_spheres;
        let mut defender_total_mana = 0_i32;
        let mut defender_total_max_mana = 0_i32;
        let defender_target_count = defenders.len().min(3);
        for (index, &slot) in defenders.iter().take(defender_target_count).enumerate() {
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
            // `sub_5FF50` gates on the owner's aggregate *current* defender
            // mana (`dword_0x13C_316`), not the aggregate max-mana display
            // counter (`dword_0x12A_298`). The source tests that value before
            // adding this holder's payload to the rebuilt totals below.
            if self
                .owner_mana_aggregate(snapshot.source_id_entity_index)
                .saturating_add(snapshot.mana)
                >= snapshot.mana_capacity
            {
                target_slot = castle_slot;
            } else if child.source_byte_3e % u8::try_from(defender_target_count.max(1)).unwrap_or(1)
                == 0
                && child.action == 9
                && child.mana < child.max_mana
            {
                // Source installs the Castle relation before calling
                // `sub_5F810`; a missing candidate therefore leaves this
                // holder pointed at the Castle, not at a stale prior row.
                target_slot = castle_slot;
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
                            parent_entity_index: castle_slot,
                            owner_entity_index: snapshot.owner_entity_index,
                            player_entity_index: snapshot.source_id_entity_index,
                            position: child_position,
                            model: 15,
                            action: 121,
                            mana: 0,
                            max_mana: 0,
                            life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                            max_life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                            speed: LEVEL2_CASTLE_CHILD_DEFAULT_SPEED,
                            target_sphere_slot: 0,
                            source_byte_3e: slot as u8,
                            cave_collision_cooldown: 0,
                            yaw: 512,
                            roll: 512,
                            control_flags: 0,
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

            // `sub_5FF50` drops an invalid tail relation before its root slot
            // is cleared. The source dispatcher has already disabled the
            // child row, so mirror that order here: unlink the renderable
            // source state, mark the typed child inactive, and return its
            // arena slot before clearing `array_0x5C_92[index]`.
            if let Some(inactive) = self
                .castle_children
                .iter()
                .find(|child| child.source_entity_index == slot)
                .copied()
            {
                if let Some(child) = self
                    .castle_children
                    .iter_mut()
                    .find(|child| child.source_entity_index == slot)
                {
                    child.active = false;
                }
                self.sync_castle_child_map(Level2CastleChild {
                    active: false,
                    ..inactive
                });
                self.release_slot(slot, Some(inactive.entity_rng));
            }
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
                let Some(child_slot) = self.allocate_slot() else {
                    self.advance_castle_phase(castle_slot);
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
                    model: 42,
                    action: 44,
                    mana: 0,
                    max_mana: 0,
                    life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                    max_life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                    speed: LEVEL2_CASTLE_CHILD_DEFAULT_SPEED,
                    target_sphere_slot: 0,
                    source_byte_3e: child_slot as u8,
                    cave_collision_cooldown: 0,
                    yaw: 0,
                    roll: 0,
                    control_flags: 1,
                    chain_previous_slot: 0,
                    chain_next_slot: 0,
                    source_byte_43: 0,
                    source_word_4a: 0,
                    active: true,
                };
                self.sync_castle_child_map(construction_child);
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
                            // Retain the root relation as the typed owner of
                            // the chain; exact source word32/word34 links are
                            // exposed separately below.
                            parent_entity_index: snapshot.source_entity_index,
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
                            model: 79,
                            action: 0,
                            mana: 0,
                            max_mana: 0,
                            life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                            max_life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                            speed: LEVEL2_CASTLE_CHILD_DEFAULT_SPEED,
                            target_sphere_slot: 0,
                            source_byte_3e: stage_slot as u8,
                            cave_collision_cooldown: 0,
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
                castle.shift_yaw = 0;
                castle.shift_fov = 256;
                castle.control_flags &= !0x40;
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
                self.events.push(Level2EconomyEvent::CastleBuilt {
                    source_entity_index: castle.source_entity_index,
                    owner_entity_index: castle.owner_entity_index,
                });
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
                    model: 41,
                    action: 43,
                    mana: 0,
                    max_mana: 0,
                    life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                    max_life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                    speed: LEVEL2_CASTLE_CHILD_DEFAULT_SPEED,
                    target_sphere_slot: 0,
                    source_byte_3e: child_slot as u8,
                    cave_collision_cooldown: 0,
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
                castle.build_state = 3;
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

    #[test]
    fn level_two_economy_projection_tracks_source_order_failure_rng_and_reset() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
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
        let mut expected_linked = raw_map_entries
            .iter()
            .map(|entry| entry.entity_index)
            .collect::<Vec<_>>();
        expected_linked.sort_unstable();
        assert_eq!(
            first.economy().map_chains().linked_entity_indices(),
            expected_linked
        );
        let mut expected_by_tile = BTreeMap::<u16, Vec<u16>>::new();
        for entry in raw_map_entries {
            expected_by_tile
                .entry(entry.tile)
                .or_default()
                .push(entry.entity_index);
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
                model: 42,
                action: 44,
                mana: 0,
                max_mana: 0,
                life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                max_life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                speed: LEVEL2_CASTLE_CHILD_DEFAULT_SPEED,
                target_sphere_slot: 0,
                source_byte_3e: construction.construction_slot as u8,
                cave_collision_cooldown: 0,
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
            castle_lifecycle
                .economy()
                .map_chains()
                .tracks(construction.construction_slot)
        );
        assert!(
            castle_lifecycle
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
        assert_eq!(stage_child.parent_entity_index, lifecycle_slot);
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
        assert_eq!(
            terminal_cursor_rows,
            vec![
                (116, 1, Level2DynamicArenaKind::CastleChild),
                (117, 1, Level2DynamicArenaKind::CastleController),
            ]
        );
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
            (4, 0, 1, 10_000, 20_000, 8_500, 8_500, 5)
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
        assert_eq!(
            callback_child_rows,
            vec![
                (116, 10, 42, 44, teardown_slot, 0, 0),
                (117, 10, 79, 0, teardown_slot, teardown_slot, 0),
                (128, 3, 3, 9, teardown_slot, 0, 0),
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
                (116, Level2DynamicArenaKind::CastleChild),
                (117, Level2DynamicArenaKind::CastleController),
            ]
        );
        let live_cursor_rows = teardown.economy().live_arena_cursor_rows();
        assert!(
            terminal_cursor_rows
                .iter()
                .all(|(source_slot, generation, _)| {
                    !live_cursor_rows
                        .iter()
                        .any(|row| row.source_slot == *source_slot && row.generation == *generation)
                })
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
                    116,
                    1,
                    Some((116, 2, Level2DynamicArenaKind::ManaAttraction)),
                    true,
                    false,
                ),
                (117, 1, None, false, true),
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
            vec![(128, 3, 3, 9, teardown_slot, 0, 0)]
        );

        // A final-stage teardown also completes with an exhausted arena and
        // clears the player Castle link plus the model-1 spell holder.
        let mut final_teardown = inspection
            .level2_reset_session()
            .expect("final Castle teardown session");
        let final_baseline_hash = final_teardown.economy().reset_hash();
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
            inspection.players()[0].enabled_spell_holders[2]
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
        // The qualified Level 2 reset census has active source rows 1..=114;
        // the source allocator therefore starts with the descending free
        // stack 999..=115 and pops 115, 116, ... in allocation order.
        let expected_free_stack = (115_u16..=999).rev().collect::<Vec<_>>();
        assert_eq!(split.economy().free_slots, expected_free_stack);
        let expected_slots = (115_u16..=130).collect::<Vec<_>>();
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
        assert!(
            !controller
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
        let defender = maintenance
            .economy()
            .castle_children()
            .iter()
            .find(|child| {
                child.active
                    && child.parent_entity_index == maintenance_slot
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
    fn level_two_applyevents_terminal_reclaims_castle_children() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("ApplyEvents terminal-reclaim session");
        let baseline_hash = session.economy().reset_hash();
        let models = [15_u8, 41, 42, 79];
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
                    model,
                    action: if model == 15 { 121 } else { 0 },
                    mana: 0,
                    max_mana: 0,
                    life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                    max_life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                    speed: LEVEL2_CASTLE_CHILD_DEFAULT_SPEED,
                    target_sphere_slot: 0,
                    source_byte_3e: source_entity_index as u8,
                    cave_collision_cooldown: 0,
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
        assert!(session.economy().free_slots.contains(&invalid_slot));
        assert!(
            !session
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
                .is_some_and(|child| !child.active)
        );
        assert_ne!(session.economy().reset_hash(), baseline_hash);
        session.economy_mut().reclaim_inactive_slots();
        assert!(
            !session
                .economy()
                .castle_children()
                .iter()
                .any(|child| child.source_entity_index == invalid_slot)
        );
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
            let castle = economy
                .castle
                .as_mut()
                .expect("defender arbitration Castle");
            // The source checks aggregate current defender mana *before*
            // adding this holder, so a one-holder fixture reaches the Castle
            // branch at the zero-capacity boundary.
            castle.mana_capacity = 0;
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
        assert!(
            session
                .economy()
                .spheres()
                .iter()
                .any(|sphere| sphere.source_entity_index == sphere_slot)
        );
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
        let chain_castle_slot = chain
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("controller-chain Castle");
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
        let defender_slots = chain
            .economy()
            .castle_children()
            .iter()
            .filter(|child| {
                child.active
                    && child.parent_entity_index == chain_castle_slot
                    && child.model == 3
                    && child.action == 9
            })
            .map(|child| child.source_entity_index)
            .collect::<Vec<_>>();
        let auxiliary_slots = chain
            .economy()
            .castle_children()
            .iter()
            .filter(|child| {
                child.active
                    && child.parent_entity_index == chain_castle_slot
                    && child.source_class == 5
                    && child.model == 15
            })
            .map(|child| child.source_entity_index)
            .collect::<Vec<_>>();
        assert!(!defender_slots.is_empty());
        assert!(!auxiliary_slots.is_empty());
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
            assert!(!chain.economy().map_chains().is_linked(*slot));
            assert!(chain.economy().free_slots.contains(slot));
        }
        let old_slot_states = old_controller_slots
            .iter()
            .map(|slot| {
                (
                    *slot,
                    chain
                        .economy()
                        .castle_children()
                        .iter()
                        .find(|child| child.active && child.source_entity_index == *slot)
                        .map(|child| (child.source_class, child.model, child.action)),
                    chain.economy().free_slots.contains(slot),
                    chain.economy().map_chains().is_linked(*slot),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            old_slot_states,
            vec![
                (117, None, true, false),
                (119, None, true, false),
                (120, None, true, false),
                (121, None, true, false),
            ]
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
    }

    #[test]
    fn level_two_mana_sphere_vertical_velocity_uses_signed_source_adjustment() {
        assert_eq!(level2_mana_sphere_vertical_velocity(0), 127);
        assert_eq!(level2_mana_sphere_vertical_velocity(1_024), 0);
        assert_eq!(level2_mana_sphere_vertical_velocity(2_048), -128);
    }
}
