//! SessionRecord state, projection, rendering, event, and audio preparation.
//!
//! This family is a pure move from the native_v2 facade. Crate visibility on
//! its internal helpers preserves the existing parent and sibling call graph.

use super::*;

impl SessionRecord {
    pub(crate) fn set_game_paused(&mut self, paused: bool) {
        if paused && !self.game_paused {
            // Capture the current live player state once at the pause
            // boundary so presentation may continue while entity action
            // dispatch remains held.
            self.camera_history = CameraHistory::capture(
                &self.state,
                self.source_turn,
                self.camera_fov,
                self.camera_look_yaw,
                self.camera_look_pitch,
            );
            self.camera_slope_sample = self.camera_slope.update(
                self.terrain.ground_heights(),
                self.camera_history.x,
                self.camera_history.y,
            );
        }
        self.game_paused = paused;
    }

    pub(crate) fn apply_presentation_input(
        &mut self,
        input: McLevel1PresentationInputV2,
    ) -> AbiResult<()> {
        validate_reserved(&input.reserved, "Level 1 presentation input")?;
        if input.reserved_0 != 0 || input.reserved_1 != 0 || input.reserved_2 != 0 {
            return Err(AbiError::new(
                MC_STATUS_RESERVED_NONZERO,
                "Level 1 presentation input reserved fields must be zero",
            ));
        }
        if input.authority_mask & !LIVE_MENU_AUTH_ALL != 0 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                format!(
                    "Level 1 presentation authority mask {:#x} has unknown bits",
                    input.authority_mask
                ),
            ));
        }
        if input.debug_heap_size != 0
            || input.debug_memory_used != 0
            || input.debug_memory_free != 0
        {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "Level 1 source allocator metrics are simulation-owned and input fields must be zero",
            ));
        }
        for (name, value) in [
            ("pause_menu_suppressed", input.pause_menu_suppressed),
            ("pause_text_suppressed", input.pause_text_suppressed),
            ("level_end_text_suppressed", input.level_end_text_suppressed),
            ("recursive_pause_writer", input.recursive_pause_writer),
            ("load_available", input.load_available),
            ("sound_available", input.sound_available),
            ("music_available", input.music_available),
            ("cursor_visible", input.cursor_visible),
        ] {
            if value > 1 {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("{name} must be zero or one"),
                ));
            }
        }
        let volume_preview = self.mission.player_menu_state == 10
            && input.volume_target == 1
            && self.presentation_ui.sound_volume != input.sound_volume;
        let chat_lines = std::array::from_fn(|index| {
            let start = index * 64;
            input.chat_lines[start..start + 64]
                .try_into()
                .expect("fixed chat line extent")
        });
        let chat_names = std::array::from_fn(|index| {
            let start = index * 16;
            input.chat_names[start..start + 16]
                .try_into()
                .expect("fixed chat name extent")
        });
        self.presentation_ui = LiveMenuState {
            authority_mask: input.authority_mask,
            selected_menu_item: input.selected_menu_item,
            pause_menu_suppressed: input.pause_menu_suppressed != 0,
            pause_text_suppressed: input.pause_text_suppressed != 0,
            level_end_text_suppressed: input.level_end_text_suppressed != 0,
            recursive_pause_writer: input.recursive_pause_writer != 0,
            load_available: input.load_available != 0,
            sound_available: input.sound_available != 0,
            music_available: input.music_available != 0,
            options_enabled_mask: input.options_enabled_mask,
            options_selected_mask: input.options_selected_mask,
            sound_volume: input.sound_volume,
            music_volume: input.music_volume,
            volume_target: input.volume_target,
            chat_recipient: input.chat_recipient,
            chat_mode: input.chat_mode,
            chat_player_mask: input.chat_player_mask,
            local_player_index: input.local_player_index,
            frame_timing_index: input.frame_timing_index,
            chat_lines,
            chat_names,
            notification_player_name: input.notification_player_name,
            cursor_visible: input.cursor_visible != 0,
            mouse_x: input.mouse_x,
            mouse_y: input.mouse_y,
            joystick: input.joystick,
            text_colour_phase: input.text_colour_phase,
        };
        if input.authority_mask & LIVE_MENU_AUTH_DEBUG_METRICS != 0 {
            if self.debug_clock_bucket != Some(input.debug_clock_bucket) {
                if self.debug_clock_bucket.is_some() {
                    self.debug_fps = self.source_turn.wrapping_sub(self.debug_previous_turn);
                }
                self.debug_clock_bucket = Some(input.debug_clock_bucket);
                self.debug_previous_turn = self.source_turn;
            }
            self.sound_number = input.sound_number;
            self.debug_heap_size = LEVEL1_SOURCE_HEAP_SIZE;
            self.debug_memory_used = LEVEL1_SOURCE_MEMORY_USED;
            self.debug_memory_free = LEVEL1_SOURCE_MEMORY_FREE;
        } else {
            self.debug_clock_bucket = None;
            self.debug_previous_turn = self.source_turn;
            self.debug_fps = 0;
            self.sound_number = 0;
            self.debug_heap_size = 0;
            self.debug_memory_used = 0;
            self.debug_memory_free = 0;
        }
        if volume_preview {
            self.emit_local_flight_sound(sim_core::volume_preview_audio_event(
                self.state.entity_index,
                i16::from(self.world.runtime().local_player_slot),
            ));
            // `AdjustVolume_1A070` synchronously calls PlayEntitySounds after
            // preparing its preview, including while byte_38591 selects the
            // sound slider during pause.
            self.drain_pending_audio_commands();
        }
        Ok(())
    }

    pub(crate) fn emit_local_flight_sound(&mut self, event: FlightEvent) {
        let record = self.encode_flight_event(event, state_hash(&self.state));
        self.append_event(record);
    }

    pub(crate) fn live_world_hash_v2(&self) -> u64 {
        self.live_world.hash_v2(self.mission.global_rng())
    }

    pub(crate) fn source_render_input_hash(&self) -> AbiResult<u64> {
        let map_right_viewport = matches!(self.mission.player_menu_state, 6 | 7 | 8 | 11 | 12 | 14);
        let inputs = self.source_render_inputs(map_right_viewport)?;
        self.source_render_input_hash_for(&inputs)
    }

    pub(crate) fn source_render_input_hash_for(
        &self,
        inputs: &SourceRenderInputs,
    ) -> AbiResult<u64> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"mc2-source-render-input-v1\0");
        bytes.extend_from_slice(&self.terrain.deterministic_hash().to_le_bytes());
        bytes.extend_from_slice(&self.live_world_hash_v2().to_le_bytes());
        bytes.push(u8::from(inputs.map_right_viewport));
        bytes.extend_from_slice(&inputs.camera.render_x.to_le_bytes());
        bytes.extend_from_slice(&inputs.camera.render_y.to_le_bytes());
        bytes.extend_from_slice(&inputs.camera.eye_z.to_le_bytes());
        bytes.extend_from_slice(&inputs.camera.yaw.to_le_bytes());
        bytes.extend_from_slice(&inputs.camera.pitch.to_le_bytes());
        bytes.extend_from_slice(&inputs.camera.roll.to_le_bytes());
        bytes.extend_from_slice(&inputs.camera.fov.to_le_bytes());
        bytes.extend_from_slice(&inputs.camera.source_turn.to_le_bytes());
        bytes.push(u8::from(inputs.camera.reflections));
        bytes.extend_from_slice(&inputs.generation_asset_digest.to_le_bytes());
        bytes.extend_from_slice(&inputs.local_player_entity_index.to_le_bytes());
        for value in [
            self.camera_slope_sample.target_x,
            self.camera_slope_sample.target_y,
            self.camera_slope_sample.offset_x,
            self.camera_slope_sample.offset_y,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(
            &u32::try_from(inputs.frozen_actors.len())
                .expect("frozen render count fits u32")
                .to_le_bytes(),
        );
        for actor in &inputs.frozen_actors {
            bytes.extend_from_slice(&actor.source_entity_index.to_le_bytes());
            bytes.extend_from_slice(&actor.position.x.to_le_bytes());
            bytes.extend_from_slice(&actor.position.y.to_le_bytes());
            bytes.extend_from_slice(&actor.position.z.to_le_bytes());
            bytes.extend_from_slice(&actor.yaw.to_le_bytes());
            bytes.extend_from_slice(&actor.render_parameter_index.to_le_bytes());
        }
        append_source_render_records(&mut bytes, b"goats", &inputs.goats);
        append_source_render_records(&mut bytes, b"ambient", &inputs.ambient_entities);
        append_source_render_records(&mut bytes, b"actors", &inputs.mission_actors);
        append_source_render_records(&mut bytes, b"residents", &inputs.dynamic_residents);
        append_source_render_records(&mut bytes, b"segments", &inputs.mission_segments);
        append_source_render_records(&mut bytes, b"castle", inputs.castle_root.as_slice());
        append_source_render_records(&mut bytes, b"spheres", &inputs.mana_spheres);
        append_source_render_records(&mut bytes, b"carrier", inputs.mana_carrier.as_slice());
        append_source_render_records(&mut bytes, b"projectiles", &inputs.projectiles);
        append_source_render_records(&mut bytes, b"impacts", &inputs.impacts);
        append_source_render_records(&mut bytes, b"possessed", &inputs.possessed_buildings);
        append_source_render_records(&mut bytes, b"source-slots", &inputs.source_slots);
        append_source_render_records(&mut bytes, b"dispositions", &inputs.dispositions);
        let topology = inputs.map_chains.canonical_topology_bytes();
        bytes.extend_from_slice(
            &u32::try_from(topology.len())
                .expect("source map topology fits u32")
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&topology);
        bytes.push(u8::from(inputs.night));
        let mut frame = |selector: u16,
                         state: u8,
                         frame_index: u8,
                         cursor: u32,
                         width: u16,
                         height: u16,
                         pixels: &[u8]| {
            bytes.extend_from_slice(&selector.to_le_bytes());
            bytes.push(state);
            bytes.push(frame_index);
            bytes.extend_from_slice(&cursor.to_le_bytes());
            bytes.extend_from_slice(&width.to_le_bytes());
            bytes.extend_from_slice(&height.to_le_bytes());
            bytes.extend_from_slice(
                &u32::try_from(pixels.len())
                    .expect("TMAP payload fits u32")
                    .to_le_bytes(),
            );
            bytes.extend_from_slice(pixels);
        };
        for clock in self
            .selector_frames
            .as_ref()
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "source-render input has no Goat TMAP clocks",
                )
            })?
            .states()
        {
            frame(
                clock.selector(),
                clock.state_ordinal(),
                clock.frame_index(),
                clock.stream_cursor(),
                clock.width(),
                clock.height(),
                clock.pixels(),
            );
        }
        for clock in self
            .archer_selector_frames
            .as_ref()
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "source-render input has no Archer TMAP clocks",
                )
            })?
            .states()
        {
            frame(
                clock.selector(),
                clock.state_ordinal(),
                clock.frame_index(),
                clock.stream_cursor(),
                clock.width(),
                clock.height(),
                clock.pixels(),
            );
        }
        for clock in self
            .firefly_selector_frames
            .as_ref()
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "source-render input has no Firefly TMAP clocks",
                )
            })?
            .states()
        {
            frame(
                clock.selector(),
                clock.state_ordinal(),
                clock.frame_index(),
                clock.stream_cursor(),
                clock.width(),
                clock.height(),
                clock.pixels(),
            );
        }
        for clock in self
            .villager_selector_frames
            .as_ref()
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "source-render input has no villager TMAP clocks",
                )
            })?
            .states()
        {
            frame(
                clock.selector(),
                clock.state_ordinal(),
                0,
                0,
                clock.width(),
                clock.height(),
                clock.pixels(),
            );
        }
        for animated in [
            self.fireball_selector_frame
                .as_ref()
                .map(FireballTmapClock::frame),
            self.possession_selector_frame
                .as_ref()
                .map(PossessionTmapClock::frame),
        ] {
            let animated = animated.ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "source-render input has no combat TMAP clock",
                )
            })?;
            frame(
                animated.selector,
                animated.state_ordinal,
                animated.frame_index,
                animated.stream_cursor,
                animated.width,
                animated.height,
                &animated.pixels,
            );
        }
        let castle = self.castle_selector_frame.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "source-render input has no Castle TMAP clock",
            )
        })?;
        let castle_frame = castle.frame();
        frame(
            castle_frame.selector,
            castle.state_ordinal(),
            0,
            0,
            castle_frame.width,
            castle_frame.height,
            &castle_frame.pixels,
        );
        Ok(sim_core::presentation_frame::byte_hash(&bytes))
    }

    pub(crate) fn new(world: NativeWorld) -> AbiResult<Self> {
        let terrain_frame_assets = load_optional_terrain_frame_assets()?.map(Arc::new);
        Self::new_with_terrain_frame_assets(world, terrain_frame_assets)
    }

    pub(crate) fn new_with_terrain_frame_assets(
        world: NativeWorld,
        terrain_frame_assets: Option<Arc<TerrainFrameAssets>>,
    ) -> AbiResult<Self> {
        let world = Arc::new(world);
        let archer_selector_frames = terrain_frame_assets
            .as_ref()
            .map(|assets| ArcherTmapClocks::from_bank(&assets.archer_tmap_bank));
        let firefly_selector_frames = terrain_frame_assets
            .as_ref()
            .map(|assets| FireflyTmapClocks::from_bank(&assets.firefly_tmap_bank));
        let villager_selector_frames = terrain_frame_assets
            .as_ref()
            .map(|assets| VillagerTmapClocks::from_bank(&assets.frozen_actor_tmap_bank));
        let fireball_selector_frame = terrain_frame_assets
            .as_ref()
            .map(|assets| FireballTmapClock::from_bank(&assets.combat_tmap_bank));
        let possession_selector_frame = terrain_frame_assets
            .as_ref()
            .map(|assets| PossessionTmapClock::from_bank(&assets.combat_tmap_bank));
        let castle_selector_frame = terrain_frame_assets
            .as_ref()
            .map(|assets| CastleTmapClock::from_bank(&assets.economy_tmap_bank));
        let frozen_actors = level1_frozen_actor_renders(world.as_ref())?;
        let renderable_map_chains =
            reset_renderable_map_chains(world.as_ref(), frozen_actors.as_ref())?;
        let player = &world.players()[usize::from(world.runtime().local_player_slot)];
        let record = player.record_bytes();
        let source_turn = player.turn;
        let camera_look_yaw = i16::from_le_bytes([record[28], record[29]]);
        let camera_look_pitch = i16::from_le_bytes([record[30], record[31]]);
        let state = *world.reset_state();
        let mut mission = world.level1_mission().session(
            world.entities()[usize::from(state.entity_index)].life,
            1_000,
        );
        mission.set_source_turn_counter(source_turn.cast_unsigned());
        mission.set_reset_source_map_chains(world.reset_source_map_chains().clone());
        let camera_fov = 128;
        let camera_history = CameraHistory::capture(
            &state,
            source_turn,
            camera_fov,
            camera_look_yaw,
            camera_look_pitch,
        );
        let mut camera_slope = TerrainSlopeCamera::default();
        let camera_slope_sample = camera_slope.update(
            world.terrain().ground_heights(),
            camera_history.x,
            camera_history.y,
        );
        let terrain = SessionTerrain::from_world(world.as_ref());
        let classic_hd_shading = Arc::from(terrain.planes().shading.clone());
        let lighting_receipt = mission
            .dynamic_lighting_execution_receipt(&terrain.planes().shading)
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
        let source_render_asset_digest = terrain_frame_assets
            .as_ref()
            .map(|assets| source_render_asset_digest(world.as_ref(), assets))
            .transpose()?;
        Ok(Self {
            terrain,
            state,
            context: world.session_context(),
            live_world: world.reset_live_world().clone(),
            mission,
            frozen_actors,
            selector_frames: world.goat_tmap_bank().map(GoatTmapClocks::from_bank),
            archer_selector_frames,
            firefly_selector_frames,
            villager_selector_frames,
            fireball_selector_frame,
            possession_selector_frame,
            castle_selector_frame,
            renderable_map_chains,
            selector_draw_feedback: None,
            notification_draw_feedback: None,
            goat_draw_admission: None,
            world,
            generation: 1,
            event_head: 0,
            events: VecDeque::new(),
            #[cfg(test)]
            event_capacity: EVENT_CAPACITY,
            audio_commands: BTreeMap::new(),
            audio_voices: [None; AUDIO_VOICE_COUNT],
            music_start_armed: true,
            fire_sequence: 0,
            last_resolution: 0,
            source_turn,
            camera_fov,
            camera_look_yaw,
            camera_look_pitch,
            camera_history,
            camera_slope,
            camera_slope_sample,
            terrain_frame_assets,
            classic_hd_shading,
            source_render_asset_digest,
            presentation_frame: PresentationFrameState::new(1),
            classic_hd_graph: None,
            classic_hd_profile_id: None,
            classic_hd_sprite_plan: None,
            classic_hd_last_world_receipt: None,
            lighting_receipt,
            presentation_ui: LiveMenuState::default(),
            debug_clock_bucket: None,
            debug_previous_turn: source_turn,
            debug_fps: 0,
            sound_number: 0,
            game_timer_turn: 0,
            debug_heap_size: 0,
            debug_memory_used: 0,
            debug_memory_free: 0,
            game_paused: false,
            pending_house_effects: Vec::new(),
            #[cfg(test)]
            immediate_released_holder_handoff: Vec::new(),
        })
    }

    pub(crate) fn reset(&mut self) {
        self.terrain = SessionTerrain::from_world(self.world.as_ref());
        Arc::make_mut(&mut self.classic_hd_shading).copy_from_slice(&self.terrain.planes().shading);
        self.state = *self.world.reset_state();
        self.context = self.world.session_context();
        self.live_world = self.world.reset_live_world().clone();
        self.mission = self.world.level1_mission().session(
            self.world.entities()[usize::from(self.state.entity_index)].life,
            1_000,
        );
        self.mission
            .set_reset_source_map_chains(self.world.reset_source_map_chains().clone());
        self.selector_frames = self.world.goat_tmap_bank().map(GoatTmapClocks::from_bank);
        self.archer_selector_frames = self
            .terrain_frame_assets
            .as_ref()
            .map(|assets| ArcherTmapClocks::from_bank(&assets.archer_tmap_bank));
        self.firefly_selector_frames = self
            .terrain_frame_assets
            .as_ref()
            .map(|assets| FireflyTmapClocks::from_bank(&assets.firefly_tmap_bank));
        self.villager_selector_frames = self
            .terrain_frame_assets
            .as_ref()
            .map(|assets| VillagerTmapClocks::from_bank(&assets.frozen_actor_tmap_bank));
        self.fireball_selector_frame = self
            .terrain_frame_assets
            .as_ref()
            .map(|assets| FireballTmapClock::from_bank(&assets.combat_tmap_bank));
        self.possession_selector_frame = self
            .terrain_frame_assets
            .as_ref()
            .map(|assets| PossessionTmapClock::from_bank(&assets.combat_tmap_bank));
        self.castle_selector_frame = self
            .terrain_frame_assets
            .as_ref()
            .map(|assets| CastleTmapClock::from_bank(&assets.economy_tmap_bank));
        self.frozen_actors = level1_frozen_actor_renders(self.world.as_ref())
            .expect("validated world reset actor renders remain valid");
        self.renderable_map_chains =
            reset_renderable_map_chains(self.world.as_ref(), self.frozen_actors.as_ref())
                .expect("validated world reset renderable chains remain valid");
        self.selector_draw_feedback = None;
        self.notification_draw_feedback = None;
        self.goat_draw_admission = None;
        self.generation = next_nonzero(self.generation);
        self.presentation_frame.reset(self.generation);
        self.classic_hd_graph = None;
        self.classic_hd_profile_id = None;
        self.classic_hd_sprite_plan = None;
        self.classic_hd_last_world_receipt = None;
        self.event_head = 0;
        self.events.clear();
        self.audio_commands.clear();
        self.audio_voices = [None; AUDIO_VOICE_COUNT];
        self.music_start_armed = true;
        self.fire_sequence = 0;
        self.last_resolution = 0;
        self.presentation_ui = LiveMenuState::default();
        self.debug_clock_bucket = None;
        self.debug_previous_turn = self.source_turn;
        self.debug_fps = 0;
        self.sound_number = 0;
        self.game_timer_turn = 0;
        self.debug_heap_size = 0;
        self.debug_memory_used = 0;
        self.debug_memory_free = 0;
        self.game_paused = false;
        let player = &self.world.players()[usize::from(self.world.runtime().local_player_slot)];
        let record = player.record_bytes();
        self.source_turn = player.turn;
        self.mission
            .set_source_turn_counter(self.source_turn.cast_unsigned());
        self.lighting_receipt = self
            .mission
            .dynamic_lighting_execution_receipt(&self.terrain.planes().shading)
            .expect("reset dynamic-light state and authored shading have fixed extents");
        self.camera_fov = 128;
        self.camera_look_yaw = i16::from_le_bytes([record[28], record[29]]);
        self.camera_look_pitch = i16::from_le_bytes([record[30], record[31]]);
        self.camera_history = CameraHistory::capture(
            &self.state,
            self.source_turn,
            self.camera_fov,
            self.camera_look_yaw,
            self.camera_look_pitch,
        );
        self.camera_slope = TerrainSlopeCamera::default();
        self.camera_slope_sample = self.camera_slope.update(
            self.terrain.ground_heights(),
            self.camera_history.x,
            self.camera_history.y,
        );
        self.pending_house_effects.clear();
        #[cfg(test)]
        self.immediate_released_holder_handoff.clear();
    }

    pub(crate) fn presentation_state(&self) -> McPresentationStateV2 {
        let camera = self.camera_history;
        McPresentationStateV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McPresentationStateV2>(),
            generation: self.generation,
            tick: camera.tick,
            source_turn: camera.source_turn,
            camera_x: camera.x,
            camera_y: camera.y,
            camera_z: camera.z,
            camera_yaw: camera.yaw,
            camera_pitch: camera.pitch,
            camera_roll: camera.roll,
            camera_fov: camera.fov,
            camera_authority: MC_AUTHORITY_PLAYER_FLIGHT_LIVE,
            camera_flags: MC_PRESENTATION_CAMERA_EXACT_V2_INPUTS
                | MC_PRESENTATION_CAMERA_EYE_Z_PLUS_128,
            reserved_0: 0,
            reserved: [0; 4],
        }
    }

    pub(crate) fn render_camera_state(&self) -> McRenderCameraStateV2 {
        let camera = self.camera_history;
        let slope = self.camera_slope_sample;
        McRenderCameraStateV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McRenderCameraStateV2>(),
            generation: self.generation,
            tick: camera.tick,
            source_turn: camera.source_turn,
            history_x: camera.x,
            history_y: camera.y,
            eye_z: camera.z,
            yaw: camera.yaw & 0x07ff,
            pitch: camera.pitch,
            roll: camera.roll,
            fov: camera.fov,
            slope_target_x: slope.target_x,
            slope_target_y: slope.target_y,
            render_offset_x: slope.offset_x,
            render_offset_y: slope.offset_y,
            authority: MC_AUTHORITY_PLAYER_FLIGHT_LIVE,
            flags: MC_RENDER_CAMERA_TERRAIN_SLOPE_XY,
            reserved_0: 0,
            reserved: [0; 4],
        }
    }

    pub(crate) fn terrain_grid(&self) -> [McTerrainGridVertexV2; OUTDOOR_GRID_VERTICES] {
        let camera = self.camera_history;
        let slope = self.camera_slope_sample;
        let planes = self.terrain.planes();
        let grid = outdoor_grid(OutdoorGridInput {
            render_x: camera.x.wrapping_add(slope.offset_x.cast_unsigned()),
            render_y: camera.y.wrapping_add(slope.offset_y.cast_unsigned()),
            yaw: camera.yaw,
            turn: camera.source_turn,
            shading: &planes.shading,
            angles: &planes.angles,
        });
        std::array::from_fn(|index| {
            let source = grid[index];
            McTerrainGridVertexV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McTerrainGridVertexV2>(),
                index: u16::try_from(index).expect("terrain grid index fits u16"),
                row: u8::try_from(index / 40).expect("terrain grid row fits u8"),
                column: u8::try_from(index % 40).expect("terrain grid column fits u8"),
                tile_x: source.tile_x,
                tile_y: source.tile_y,
                shade: source.shade,
                angle: source.angle,
                view_x: source.view_x,
                view_depth: source.view_depth,
                distance_squared: source.distance_squared,
                pnt5: source.pnt5.unwrap_or(0),
                raw_wave: source.raw_wave,
                surface_displacement: source.surface_displacement,
                lighting_wave: source.lighting_wave,
                feature_flags: source.feature_flags,
                tablesn_row: source.vertex_tablesn_row.unwrap_or(0),
                pnt5_written: u8::from(source.pnt5.is_some()),
                reserved: [0; 4],
            }
        })
    }

    pub(crate) fn terrain_frame(&self) -> AbiResult<Level1TerrainFrame> {
        let assets = self.terrain_frame_assets.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "isolated terrain frame unavailable: MC_ASSET_CACHE_PATH was not supplied when the session opened",
            )
        })?;
        let camera = self.camera_history;
        let slope = self.camera_slope_sample;
        let planes = self.terrain.planes();
        render_level1_terrain_frame(
            Level1TerrainFrameCamera {
                render_x: camera.x.wrapping_add(slope.offset_x.cast_unsigned()),
                render_y: camera.y.wrapping_add(slope.offset_y.cast_unsigned()),
                eye_z: i32::from(camera.z),
                yaw: camera.yaw,
                pitch: camera.pitch,
                roll: camera.roll,
                fov: camera.fov,
                source_turn: camera.source_turn,
                reflections: true,
            },
            Level1TerrainFrameResources {
                terrain_types: &planes.terrain_types,
                ground_heights: &planes.ground_heights,
                shading: &planes.shading,
                angles: &planes.angles,
                terrain_atlas: &assets.terrain_atlas,
                tablesn: &assets.tablesn,
                sky: &assets.sky,
            },
        )
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))
    }

    pub(crate) fn level1_palette_rgb8(&self) -> AbiResult<[u8; 768]> {
        let assets = self.terrain_frame_assets.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "Level 1 palette unavailable: MC_ASSET_CACHE_PATH was not supplied when the session opened",
            )
        })?;
        let original: &[u8; 768] = assets.palette_rgb6.as_ref().try_into().map_err(|_| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "qualified PALN RGB6 palette must contain exactly 768 bytes",
            )
        })?;
        let rgb6 = self.mission.effective_palette_rgb6(original);
        Ok(rgb6.map(|channel| channel.saturating_mul(4)))
    }

    pub(crate) fn render_world_frame(
        &self,
        ambient_entities: &[Level1AmbientEntity],
        map_chains: &RenderableMapChains,
    ) -> AbiResult<Level1TerrainFrame> {
        let unavailable = |message| AbiError::new(MC_STATUS_VALIDATION_FAILED, message);
        let assets = self.terrain_frame_assets.as_ref().ok_or_else(|| {
            unavailable(
                "world frame unavailable: MC_ASSET_CACHE_PATH was not supplied when the session opened",
            )
        })?;
        let tmap_clocks = self.selector_frames.as_ref().ok_or_else(|| {
            unavailable("world frame unavailable without qualified Goat TMAP descriptors")
        })?;
        let archer_tmap_clocks = self.archer_selector_frames.as_ref().ok_or_else(|| {
            unavailable("world frame unavailable without qualified Archer TMAP descriptors")
        })?;
        let firefly_tmap_clocks = self.firefly_selector_frames.as_ref().ok_or_else(|| {
            unavailable("world frame unavailable without qualified Firefly TMAP descriptors")
        })?;
        let villager_tmap_clocks = self.villager_selector_frames.as_ref().ok_or_else(|| {
            unavailable("world frame unavailable without qualified villager TMAP descriptors")
        })?;
        let fireball_tmap_clock = self.fireball_selector_frame.as_ref().ok_or_else(|| {
            unavailable("world frame unavailable without qualified Fireball TMAP descriptor")
        })?;
        let possession_tmap_clock = self.possession_selector_frame.as_ref().ok_or_else(|| {
            unavailable("world frame unavailable without qualified Possession TMAP descriptor")
        })?;
        let castle_tmap_clock = self.castle_selector_frame.as_ref().ok_or_else(|| {
            unavailable("world frame unavailable without qualified Castle TMAP descriptor")
        })?;
        let camera = self.camera_history;
        let slope = self.camera_slope_sample;
        let planes = self.terrain.planes();
        let dynamic_residents = self
            .mission
            .dynamic_residents()
            .copied()
            .collect::<Vec<_>>();
        render_level1_world_frame(
            Level1TerrainFrameCamera {
                render_x: camera.x.wrapping_add(slope.offset_x.cast_unsigned()),
                render_y: camera.y.wrapping_add(slope.offset_y.cast_unsigned()),
                eye_z: i32::from(camera.z),
                yaw: camera.yaw,
                pitch: camera.pitch,
                roll: camera.roll,
                fov: camera.fov,
                source_turn: camera.source_turn,
                reflections: true,
            },
            Level1TerrainFrameResources {
                terrain_types: &planes.terrain_types,
                ground_heights: &planes.ground_heights,
                shading: &planes.shading,
                angles: &planes.angles,
                terrain_atlas: &assets.terrain_atlas,
                tablesn: &assets.tablesn,
                sky: &assets.sky,
            },
            Level1WorldFrameResources {
                local_player_entity_index: self.state.entity_index,
                goats: self.live_world.goats(),
                frozen_actors: &self.frozen_actors,
                ambient_entities,
                frozen_actor_tmap_bank: Some(&assets.frozen_actor_tmap_bank),
                villager_tmap_clocks: Some(villager_tmap_clocks),
                mission_actors: self.mission.actors(),
                dynamic_residents: &dynamic_residents,
                mission_segments: self.mission.multipart_segments(),
                castle_root: self
                    .mission
                    .castle_exists
                    .then_some(Level1CastleRootRender {
                        source_entity_index: self.mission.castle_source_entity_index,
                        position: self.mission.castle_position,
                        active: true,
                    }),
                mana_spheres: self.mission.mana_spheres(),
                mana_carrier: self.mission.mana_carrier(),
                map_chains,
                goat_tmap_clocks: tmap_clocks,
                archer_tmap_bank: &assets.archer_tmap_bank,
                archer_tmap_clocks,
                firefly_tmap_bank: &assets.firefly_tmap_bank,
                firefly_tmap_clocks,
                projectiles: self.mission.projectiles(),
                impacts: self.mission.impacts(),
                possessed_buildings: self.mission.possessed_buildings(),
                combat_tmap_bank: &assets.combat_tmap_bank,
                static_world_tmap_bank: &assets.static_world_tmap_bank,
                economy_tmap_bank: &assets.economy_tmap_bank,
                fireball_tmap_clock,
                possession_tmap_clock,
                castle_tmap_clock,
                particle_parameters: &self.world.presentation().particles,
                source_slots: self.mission.source_slots(),
                night: self.world.metadata().map_type == sim_core::native_world::MapType::Night,
            },
        )
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))
    }

    pub(crate) fn source_sprite_dispositions(&self) -> AbiResult<Vec<SourceSpriteDisposition>> {
        let mut dispositions = self
            .mission
            .source_sprite_dispositions(&self.state)
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
        let mut qualified = dispositions
            .iter()
            .map(source_sprite_disposition_index)
            .collect::<BTreeSet<_>>();
        let legacy = self
            .mission
            .source_sprite_legacy_dispositions(&qualified)
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
        qualified.extend(legacy.iter().map(source_sprite_disposition_index));
        dispositions.extend(legacy);

        let mut imported_rows = Vec::new();
        for source_entity_index in self.renderable_map_chains.linked_entity_indices() {
            if qualified.contains(&source_entity_index) {
                continue;
            }
            let entity = self
                .world
                .entities()
                .get(usize::from(source_entity_index))
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        unknown_source_sprite_tuple(source_entity_index, 0, 0, 0, 0),
                    )
                })?;
            let source = self
                .mission
                .source_slots()
                .get(usize::from(source_entity_index))
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        unknown_source_sprite_tuple(source_entity_index, 0, 0, 0, 0),
                    )
                })?;
            let route = if entity.authority == EntityAuthority::GoatLive {
                if (source.class, source.model) != (GOAT_CLASS, GOAT_MODEL)
                    || !matches!(source.action, 9 | 12 | 13 | 14 | GOAT_PATROL_ACTION)
                {
                    return Err(AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        format!(
                            "live Goat source row mismatch: slot={source_entity_index} source={}/{}/{} phase={} rng={} flags={:#x}",
                            source.class,
                            source.model,
                            source.action,
                            source.phase,
                            source.entity_rng,
                            source.control_flags,
                        ),
                    ));
                }
                SourceSpriteContractRoute::Legacy(SourceSpriteLegacyFamily::Goat)
            } else if entity.authority == EntityAuthority::VillagerLive {
                if (source.class, source.model) != (VILLAGER_CLASS, VILLAGER_MODEL)
                    || !matches!(source.action, 104..=111)
                {
                    return Err(AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        format!(
                            "live Villager source row mismatch: slot={source_entity_index} source={}/{}/{} phase={} rng={} flags={:#x}",
                            source.class,
                            source.model,
                            source.action,
                            source.phase,
                            source.entity_rng,
                            source.control_flags,
                        ),
                    ));
                }
                SourceSpriteContractRoute::Legacy(SourceSpriteLegacyFamily::Resident)
            } else {
                let tuple = ImportedResetSpriteTuple {
                    class: source.class,
                    model: source.model,
                    action: source.action,
                    entity_parameter_index: entity.parameter_relation.unwrap_or(0),
                    particle_parameter_index: entity.particle_parameter_index.unwrap_or(0),
                    animation_frame: entity.animation_frame(),
                    control_flags: entity.control_flags,
                };
                match imported_reset_sprite_disposition(tuple).map_err(|_| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        unknown_source_sprite_tuple(
                            source_entity_index,
                            source.class,
                            source.model,
                            source.action,
                            entity.particle_parameter_index.unwrap_or(0),
                        ),
                    )
                })? {
                    SourceSpriteTemplateDisposition::Render(_) => {
                        SourceSpriteContractRoute::Legacy(SourceSpriteLegacyFamily::ImportedReset)
                    }
                    SourceSpriteTemplateDisposition::Excluded(reason) => {
                        SourceSpriteContractRoute::Excluded(reason)
                    }
                }
            };
            imported_rows.push(SourceSpriteContractRow {
                source_entity_index,
                position: entity.position,
                yaw: entity.yaw,
                pitch: entity.pitch,
                roll: 0,
                control_flags: source.control_flags,
                class: source.class,
                model: source.model,
                action: source.action,
                xclass: 0,
                xmodel: 0,
                route,
            });
        }
        let imported = source_sprite_dispositions(imported_rows)
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
        qualified.extend(imported.iter().map(source_sprite_disposition_index));
        dispositions.extend(imported);
        dispositions.sort_unstable_by_key(source_sprite_disposition_index);
        for source_entity_index in self.renderable_map_chains.linked_entity_indices() {
            if !qualified.contains(&source_entity_index) {
                let (class, model, action) = self
                    .mission
                    .source_slots()
                    .get(usize::from(source_entity_index))
                    .map_or((0, 0, 0), |source| {
                        (source.class, source.model, source.action)
                    });
                return Err(AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    unknown_source_sprite_tuple(source_entity_index, class, model, action, 0),
                ));
            }
        }
        Ok(dispositions)
    }

    pub(crate) fn source_render_inputs(
        &self,
        map_right_viewport: bool,
    ) -> AbiResult<SourceRenderInputs> {
        let dispositions = self.source_sprite_dispositions()?;
        let mut map_chains = self.renderable_map_chains.clone();
        for disposition in &dispositions {
            let SourceSpriteDisposition::Render(submission) = disposition else {
                continue;
            };
            if submission.source_entity_index == self.state.entity_index {
                map_chains
                    .sync_entity(RenderableMapSeed {
                        entity_index: submission.source_entity_index,
                        active: true,
                        x: submission.position.x,
                        y: submission.position.y,
                    })
                    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
            }
        }
        if self.terrain_frame_assets.is_none() {
            return Err(AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "source-render generation has no qualified asset banks",
            ));
        }
        // `TerrainFrameAssets` is held behind an immutable `Arc` for the
        // lifetime of the session. Its digest was computed and stored when
        // the owner cache opened, so recomputing a hash over the terrain,
        // TABLESN, sky, and every TMAP debug record on every HD frame adds
        // tens of milliseconds without providing a possible mutation gate.
        // Reuse the generation-bound receipt instead; cache replacement still
        // requires a session reset/open and therefore cannot silently cross a
        // generation here.
        let generation_asset_digest = self.source_render_asset_digest.ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "source-render generation has no asset digest",
            )
        })?;
        let camera = Level1TerrainFrameCamera {
            render_x: self
                .camera_history
                .x
                .wrapping_add(self.camera_slope_sample.offset_x.cast_unsigned()),
            render_y: self
                .camera_history
                .y
                .wrapping_add(self.camera_slope_sample.offset_y.cast_unsigned()),
            eye_z: i32::from(self.camera_history.z).wrapping_add(if map_right_viewport {
                128
            } else {
                0
            }),
            yaw: i16::try_from(self.camera_history.yaw.cast_unsigned() & 0x07ff)
                .expect("masked source yaw fits i16"),
            pitch: self.camera_history.pitch,
            roll: self.camera_history.roll,
            fov: self.camera_history.fov,
            source_turn: self.camera_history.source_turn,
            reflections: true,
        };
        Ok(SourceRenderInputs {
            map_right_viewport,
            camera,
            generation_asset_digest,
            local_player_entity_index: self.state.entity_index,
            goats: self.live_world.goats().to_vec(),
            frozen_actors: self.frozen_actors.to_vec(),
            ambient_entities: self.mission.ambient_entities().copied().collect(),
            mission_actors: self.mission.actors().to_vec(),
            dynamic_residents: self.mission.dynamic_residents().copied().collect(),
            mission_segments: self.mission.multipart_segments().to_vec(),
            castle_root: self
                .mission
                .castle_exists
                .then_some(Level1CastleRootRender {
                    source_entity_index: self.mission.castle_source_entity_index,
                    position: self.mission.castle_position,
                    active: true,
                }),
            mana_spheres: self.mission.mana_spheres().to_vec(),
            mana_carrier: self.mission.mana_carrier().copied(),
            map_chains,
            projectiles: self.mission.projectiles().to_vec(),
            impacts: self.mission.impacts().to_vec(),
            possessed_buildings: self.mission.possessed_buildings().to_vec(),
            source_slots: self.mission.source_slots().to_vec(),
            dispositions,
            night: self.world.metadata().map_type == sim_core::native_world::MapType::Night,
        })
    }

    pub(crate) fn render_source_world_frame(
        &self,
        inputs: &SourceRenderInputs,
    ) -> AbiResult<Level1TerrainFrame> {
        let unavailable = |message| AbiError::new(MC_STATUS_VALIDATION_FAILED, message);
        let assets = self.terrain_frame_assets.as_ref().ok_or_else(|| {
            unavailable(
                "source world frame unavailable: MC_ASSET_CACHE_PATH was not supplied when the session opened",
            )
        })?;
        let source_bank = self.world.source_sprite_tmap_bank().ok_or_else(|| {
            unavailable("source world frame unavailable without the CP1 source TMAP bank")
        })?;
        let tmap_clocks = self.selector_frames.as_ref().ok_or_else(|| {
            unavailable("source world frame unavailable without Goat TMAP clocks")
        })?;
        let archer_tmap_clocks = self.archer_selector_frames.as_ref().ok_or_else(|| {
            unavailable("source world frame unavailable without Archer TMAP clocks")
        })?;
        let firefly_tmap_clocks = self.firefly_selector_frames.as_ref().ok_or_else(|| {
            unavailable("source world frame unavailable without Firefly TMAP clocks")
        })?;
        let villager_tmap_clocks = self.villager_selector_frames.as_ref().ok_or_else(|| {
            unavailable("source world frame unavailable without villager TMAP clocks")
        })?;
        let fireball_tmap_clock = self.fireball_selector_frame.as_ref().ok_or_else(|| {
            unavailable("source world frame unavailable without Fireball TMAP clock")
        })?;
        let possession_tmap_clock = self.possession_selector_frame.as_ref().ok_or_else(|| {
            unavailable("source world frame unavailable without Possession TMAP clock")
        })?;
        let castle_tmap_clock = self.castle_selector_frame.as_ref().ok_or_else(|| {
            unavailable("source world frame unavailable without Castle TMAP clock")
        })?;
        let planes = self.terrain.planes();
        let render = if inputs.map_right_viewport {
            render_level1_source_world_map_frame_composed
        } else {
            render_level1_source_world_frame
        };
        render(
            inputs.camera,
            Level1TerrainFrameResources {
                terrain_types: &planes.terrain_types,
                ground_heights: &planes.ground_heights,
                shading: &planes.shading,
                angles: &planes.angles,
                terrain_atlas: &assets.terrain_atlas,
                tablesn: &assets.tablesn,
                sky: &assets.sky,
            },
            Level1WorldFrameResources {
                local_player_entity_index: inputs.local_player_entity_index,
                goats: &inputs.goats,
                frozen_actors: &inputs.frozen_actors,
                ambient_entities: &inputs.ambient_entities,
                frozen_actor_tmap_bank: Some(&assets.frozen_actor_tmap_bank),
                villager_tmap_clocks: Some(villager_tmap_clocks),
                mission_actors: &inputs.mission_actors,
                dynamic_residents: &inputs.dynamic_residents,
                mission_segments: &inputs.mission_segments,
                castle_root: inputs.castle_root,
                mana_spheres: &inputs.mana_spheres,
                mana_carrier: inputs.mana_carrier.as_ref(),
                map_chains: &inputs.map_chains,
                goat_tmap_clocks: tmap_clocks,
                archer_tmap_bank: &assets.archer_tmap_bank,
                archer_tmap_clocks,
                firefly_tmap_bank: &assets.firefly_tmap_bank,
                firefly_tmap_clocks,
                projectiles: &inputs.projectiles,
                impacts: &inputs.impacts,
                possessed_buildings: &inputs.possessed_buildings,
                combat_tmap_bank: &assets.combat_tmap_bank,
                static_world_tmap_bank: &assets.static_world_tmap_bank,
                economy_tmap_bank: &assets.economy_tmap_bank,
                fireball_tmap_clock,
                possession_tmap_clock,
                castle_tmap_clock,
                particle_parameters: &self.world.presentation().particles,
                source_slots: &inputs.source_slots,
                night: inputs.night,
            },
            SourceSpriteWorldResources {
                dispositions: &inputs.dispositions,
                tmap_bank: source_bank,
            },
        )
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))
    }

    /// Render the owner world directly into the profile-sized Classic HD
    /// graph scratch planes. This is deliberately separate from
    /// `render_source_world_frame`: that function remains the sealed 320x200
    /// Exact source compositor and is never used as an upscale source here.
    pub(crate) fn render_classic_hd_world(
        &mut self,
        profile: &ClassicHdRenderProfile,
        indices: &mut [u8],
        provenance: &mut [u8],
        generation: u64,
        tick: u64,
        input_hash: u64,
        history_hash: u64,
    ) -> Result<ClassicHdWorldReceipt, ClassicHdGraphError> {
        let profile_trace = std::env::var_os("MC_CLASSIC_HD_PROFILE_TRACE").is_some();
        let render_started = std::time::Instant::now();
        let assets = Arc::clone(self.terrain_frame_assets.as_ref().ok_or_else(|| {
            ClassicHdGraphError(
                "Classic HD owner render requires MC_ASSET_CACHE_PATH assets".to_owned(),
            )
        })?);
        let inputs_started = std::time::Instant::now();
        let mut inputs = self
            .source_render_inputs(false)
            .map_err(|error| ClassicHdGraphError(error.message))?;
        if profile_trace {
            eprintln!(
                "[CLASSIC HD TRACE] profile={} source_inputs_ms={} dispositions={}",
                profile.id.as_str(),
                inputs_started.elapsed().as_millis(),
                inputs.dispositions.len(),
            );
        }

        let terrain_build_started = std::time::Instant::now();
        let terrain = {
            let planes = self.terrain.planes();
            build_classic_hd_terrain(
                profile,
                inputs.camera,
                ClassicHdTerrainResources {
                    terrain_types: &planes.terrain_types,
                    ground_heights: &planes.ground_heights,
                    shading: &planes.shading,
                    angles: &planes.angles,
                    sky: &assets.sky,
                },
                ClassicHdSkyBank::Base256,
            )
            .map_err(|error| ClassicHdGraphError(error.to_string()))?
        };
        if profile_trace {
            eprintln!(
                "[CLASSIC HD TRACE] profile={} terrain_build_ms={} lattice_vertices={} submissions_reflected={} submissions_primary={}",
                profile.id.as_str(),
                terrain_build_started.elapsed().as_millis(),
                terrain.lattice.vertex_count(),
                terrain.submissions.reflected.len(),
                terrain.submissions.primary.len(),
            );
        }
        let mut writer = ClassicHdFrameWriter::from_planes(
            profile,
            indices,
            provenance,
            generation,
            tick,
            input_hash,
            history_hash,
        )
        .map_err(ClassicHdGraphError::from)?;
        writer.indices_mut().copy_from_slice(&terrain.sky);
        writer.provenance_mut().fill(0x01);
        let terrain_raster_started = std::time::Instant::now();
        let terrain_receipt = rasterize_classic_hd_terrain(
            &mut writer,
            &terrain,
            &assets.terrain_atlas,
            &assets.tablesn,
            0x10,
        )
        .map_err(|error| ClassicHdGraphError(error.to_string()))?;
        if profile_trace {
            eprintln!(
                "[CLASSIC HD TRACE] profile={} terrain_raster_ms={} index_hash={:016x}",
                profile.id.as_str(),
                terrain_raster_started.elapsed().as_millis(),
                byte_hash(writer.indices()),
            );
            eprintln!(
                "[CLASSIC HD TRACE] profile={} mode5_triangles={} mode26_triangles={} terrain_source_pixels={} terrain_written_pixels={} terrain_clipped_pixels={}",
                profile.id.as_str(),
                terrain_receipt.mode5_triangles,
                terrain_receipt.mode26_triangles,
                terrain_receipt.source_pixels,
                terrain_receipt.written_pixels,
                terrain_receipt.clipped_pixels,
            );
        }

        let disposition_started = std::time::Instant::now();
        // `source_render_inputs` already validates and orders the complete
        // disposition list. Reusing that owned list avoids rebuilding the
        // BTreeSet/imported-row admission pass a second time on every HD
        // frame; the authoritative state remains the same session snapshot.
        let dispositions = std::mem::take(&mut inputs.dispositions);
        let source_bank = self.world.source_sprite_tmap_bank().ok_or_else(|| {
            ClassicHdGraphError(
                "Classic HD owner render requires the CP1 source TMAP bank".to_owned(),
            )
        })?;
        let ground_heights = &self.terrain.planes().ground_heights;
        let mut plan = self
            .classic_hd_sprite_plan
            .take()
            .unwrap_or_else(|| ClassicHdSpritePlan::with_capacity(1_024));
        let plan_result = build_classic_hd_sprite_plan(
            profile,
            inputs.camera,
            &dispositions,
            source_bank,
            ground_heights,
            inputs.night,
            &mut plan,
        );
        if let Err(error) = plan_result {
            self.classic_hd_sprite_plan = Some(plan);
            return Err(ClassicHdGraphError(error.to_string()));
        }
        if profile_trace {
            eprintln!(
                "[CLASSIC HD TRACE] profile={} dispositions={} sprite_plan_primary={} sprite_plan_legacy={} disposition_plan_ms={}",
                profile.id.as_str(),
                dispositions.len(),
                plan.primary.len(),
                plan.legacy.len(),
                disposition_started.elapsed().as_millis(),
            );
        }

        let tmap_clocks = self.selector_frames.as_ref().ok_or_else(|| {
            ClassicHdGraphError("Classic HD owner render has no Goat TMAP clocks".to_owned())
        })?;
        let archer_tmap_clocks = self.archer_selector_frames.as_ref().ok_or_else(|| {
            ClassicHdGraphError("Classic HD owner render has no Archer TMAP clocks".to_owned())
        })?;
        let firefly_tmap_clocks = self.firefly_selector_frames.as_ref().ok_or_else(|| {
            ClassicHdGraphError("Classic HD owner render has no Firefly TMAP clocks".to_owned())
        })?;
        let villager_tmap_clocks = self.villager_selector_frames.as_ref().ok_or_else(|| {
            ClassicHdGraphError("Classic HD owner render has no villager TMAP clocks".to_owned())
        })?;
        let fireball_tmap_clock = self.fireball_selector_frame.as_ref().ok_or_else(|| {
            ClassicHdGraphError("Classic HD owner render has no Fireball TMAP clock".to_owned())
        })?;
        let possession_tmap_clock = self.possession_selector_frame.as_ref().ok_or_else(|| {
            ClassicHdGraphError("Classic HD owner render has no Possession TMAP clock".to_owned())
        })?;
        let castle_tmap_clock = self.castle_selector_frame.as_ref().ok_or_else(|| {
            ClassicHdGraphError("Classic HD owner render has no Castle TMAP clock".to_owned())
        })?;
        let planes = self.terrain.planes();
        let terrain_projection = terrain.projection;
        let projection = OutdoorProjection {
            scale: terrain_projection.scale,
            pitch_offset: terrain_projection.pitch_offset,
            roll: terrain_projection.roll,
            center_x: terrain_projection.center_x,
            center_y: terrain_projection.center_y,
            viewport_width: terrain_projection.viewport_width,
            viewport_height: terrain_projection.viewport_height,
        };
        let sprite_raster_started = std::time::Instant::now();
        let sprite_receipt = match raster_level1_source_world_sprites(
            inputs.camera,
            projection,
            Level1TerrainFrameResources {
                terrain_types: &planes.terrain_types,
                ground_heights: &planes.ground_heights,
                shading: &planes.shading,
                angles: &planes.angles,
                terrain_atlas: &assets.terrain_atlas,
                tablesn: &assets.tablesn,
                sky: &assets.sky,
            },
            Level1WorldFrameResources {
                local_player_entity_index: inputs.local_player_entity_index,
                goats: &inputs.goats,
                frozen_actors: &inputs.frozen_actors,
                ambient_entities: &inputs.ambient_entities,
                frozen_actor_tmap_bank: Some(&assets.frozen_actor_tmap_bank),
                villager_tmap_clocks: Some(villager_tmap_clocks),
                mission_actors: &inputs.mission_actors,
                dynamic_residents: &inputs.dynamic_residents,
                mission_segments: &inputs.mission_segments,
                castle_root: inputs.castle_root,
                mana_spheres: &inputs.mana_spheres,
                mana_carrier: inputs.mana_carrier.as_ref(),
                map_chains: &inputs.map_chains,
                goat_tmap_clocks: tmap_clocks,
                archer_tmap_bank: &assets.archer_tmap_bank,
                archer_tmap_clocks,
                firefly_tmap_bank: &assets.firefly_tmap_bank,
                firefly_tmap_clocks,
                projectiles: &inputs.projectiles,
                impacts: &inputs.impacts,
                possessed_buildings: &inputs.possessed_buildings,
                combat_tmap_bank: &assets.combat_tmap_bank,
                static_world_tmap_bank: &assets.static_world_tmap_bank,
                economy_tmap_bank: &assets.economy_tmap_bank,
                fireball_tmap_clock,
                possession_tmap_clock,
                castle_tmap_clock,
                particle_parameters: &self.world.presentation().particles,
                source_slots: &inputs.source_slots,
                night: inputs.night,
            },
            SourceSpriteWorldResources {
                dispositions: &dispositions,
                tmap_bank: source_bank,
            },
            writer.indices_mut(),
        ) {
            Ok(receipt) => receipt,
            Err(error) => {
                self.classic_hd_sprite_plan = Some(plan);
                return Err(ClassicHdGraphError(error.to_string()));
            }
        };
        if profile_trace {
            eprintln!(
                "[CLASSIC HD TRACE] profile={} sprite_raster_ms={} primary_candidates={} primary_writes={} reflected_candidates={} shadow_candidates={} total_ms={}",
                profile.id.as_str(),
                sprite_raster_started.elapsed().as_millis(),
                sprite_receipt.primary.candidate_pixels,
                sprite_receipt.primary.framebuffer_writes,
                sprite_receipt.reflected.candidate_pixels,
                sprite_receipt.shadows.candidate_pixels,
                render_started.elapsed().as_millis(),
            );
        }

        let receipt = ClassicHdWorldReceipt {
            world_hash: byte_hash(writer.indices()),
            provenance_hash: byte_hash(writer.provenance()),
            terrain_rendered: true,
            typed_sprites_rendered: true,
            reflection_pass_rendered: sprite_receipt.reflection_pass_evaluated,
            shadow_pass_rendered: sprite_receipt.shadow_pass_evaluated,
            water_pass_rendered: false,
        };
        self.classic_hd_sprite_plan = Some(plan);
        self.classic_hd_last_world_receipt = Some(receipt);
        Ok(receipt)
    }

    pub(crate) fn validate_classic_hd_frame_request(
        &self,
        profile_id: u32,
        query: &McPresentationFrameQueryV2,
        index_buffer: &[u8],
        provenance_buffer: &[u8],
    ) -> AbiResult<ClassicHdRenderProfile> {
        let profile = classic_hd_profile_for_id(profile_id)?;
        let required = profile.extent();
        if index_buffer.len() != required || provenance_buffer.len() != required {
            return Err(AbiError::new(
                MC_STATUS_BUFFER_TOO_SMALL,
                format!(
                    "Classic HD profile {} requires {required} index and provenance bytes",
                    profile.id.as_str()
                ),
            ));
        }
        if query.generation != self.generation {
            return Err(AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                format!(
                    "Classic HD generation {} does not match {}",
                    query.generation, self.generation
                ),
            ));
        }
        if query.tick != self.camera_history.tick {
            return Err(AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                format!(
                    "Classic HD tick {} does not match camera history {}",
                    query.tick, self.camera_history.tick
                ),
            ));
        }
        if query.viewport_width != profile.width as u16
            || query.viewport_height != profile.height as u16
            || query.screen_stride != profile.stride as u16
        {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                format!(
                    "Classic HD query viewport {}x{} stride {} does not match profile {}x{} stride {}",
                    query.viewport_width,
                    query.viewport_height,
                    query.screen_stride,
                    profile.width,
                    profile.height,
                    profile.stride
                ),
            ));
        }
        if (query.game_paused != 0) != self.game_paused {
            return Err(AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "Classic HD paused query does not match authoritative session state",
            ));
        }
        Ok(profile)
    }

    pub(crate) fn classic_hd_frame_copy(
        &mut self,
        profile_id: u32,
        query: McPresentationFrameQueryV2,
        index_buffer: &mut [u8],
        provenance_buffer: &mut [u8],
    ) -> AbiResult<McPresentationFrameInfoV2> {
        let profile = self.validate_classic_hd_frame_request(
            profile_id,
            &query,
            index_buffer,
            provenance_buffer,
        )?;
        let required = profile.extent();

        let source_input_hash = self.source_render_input_hash()?;
        let mut input_bytes = Vec::with_capacity(96);
        input_bytes.extend_from_slice(b"mc2-classic-hd-owner-input-v1\0");
        input_bytes.extend_from_slice(&source_input_hash.to_le_bytes());
        input_bytes.extend_from_slice(&profile.profile_revision.to_le_bytes());
        input_bytes.extend_from_slice(&query.presentation_revision.to_le_bytes());
        input_bytes.push(query.configured_blur_mode);
        input_bytes.push(query.blur_capability);
        input_bytes.push(query.high_speed_system);
        input_bytes.push(query.scratch_present);
        input_bytes.push(query.vga_resolution);
        input_bytes.push(query.spatial_smoothing);
        input_bytes.push(query.minimap_enabled);
        input_bytes.push(query.top_bar_enabled);
        input_bytes.push(query.menu_overlay);
        input_bytes.push(query.game_paused);
        input_bytes.push(query.level_end);
        input_bytes.push(query.objective_selector);
        input_bytes.push(query.objective_textbox);
        input_bytes.push(query.tutorial_or_briefing);
        input_bytes.push(query.help_popup);
        input_bytes.push(query.debug_text);
        let input_hash = byte_hash(&input_bytes);

        if self.classic_hd_graph.is_none() || self.classic_hd_profile_id != Some(profile_id) {
            self.classic_hd_graph = Some(
                ClassicHdRenderGraph::new(profile.clone(), self.generation)
                    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.0))?,
            );
            self.classic_hd_profile_id = Some(profile_id);
            self.classic_hd_sprite_plan = Some(ClassicHdSpritePlan::with_capacity(1_024));
            self.classic_hd_last_world_receipt = None;
        }

        let mut graph = self.classic_hd_graph.take().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "Classic HD graph was not initialized",
            )
        })?;
        let had_history = graph.history_key().is_some();
        let generation = self.generation;
        let tick = self.camera_history.tick;
        let blur = ClassicHdBlurInput {
            configured_mode: query.configured_blur_mode,
            blur_capability: query.blur_capability != 0,
            high_speed_system: query.high_speed_system != 0,
            scratch_present: query.scratch_present != 0,
            menu_state: self.mission.player_menu_state,
            actual_speed: self.state.recoil_speed,
            spatial_smoothing: query.spatial_smoothing != 0,
            spatial_radius: 1,
        };
        let shading = Arc::clone(&self.classic_hd_shading);
        let assets = Arc::clone(self.terrain_frame_assets.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "Classic HD owner render requires MC_ASSET_CACHE_PATH assets",
            )
        })?);
        let graph_result = graph.render_frame(
            ClassicHdGraphInput {
                generation,
                tick,
                profile_revision: profile.profile_revision,
                input_hash,
                reflections_enabled: true,
                dynamic_lighting_enabled: true,
                lighting_path: ClassicHdLightingPath::OwnerWorldAlreadyLit,
                water_enabled: false,
                blur,
            },
            Some(shading.as_ref()),
            &assets.tablesn,
            |indices, provenance| {
                self.render_classic_hd_world(
                    &profile, indices, provenance, generation, tick, input_hash, 0,
                )
            },
        );
        self.classic_hd_graph = Some(graph);
        let result =
            graph_result.map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.0))?;
        let world_receipt = self.classic_hd_last_world_receipt.ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "Classic HD graph completed without an owner world receipt",
            )
        })?;
        index_buffer.copy_from_slice(
            self.classic_hd_graph
                .as_ref()
                .expect("Classic HD graph restored")
                .current_indices(),
        );
        provenance_buffer.copy_from_slice(
            self.classic_hd_graph
                .as_ref()
                .expect("Classic HD graph restored")
                .current_provenance(),
        );
        let (overlay_pixels_written, overlay_stage_count, _, _) = self
            .composite_classic_hd_owner_overlays(
                &profile,
                query,
                index_buffer,
                provenance_buffer,
            )?;
        let mut flags = MC_CLASSIC_HD_FRAME_PROFILE_RERENDER
            | MC_PRESENTATION_FRAME_HAS_PROVENANCE
            | MC_PRESENTATION_FRAME_OVERLAYS_COMPOSITED
            | MC_PRESENTATION_FRAME_POSTPROCESS_COMPLETE
            | MC_PRESENTATION_FRAME_LIGHTING_COMPLETE;
        if world_receipt.typed_sprites_rendered {
            flags |= MC_CLASSIC_HD_FRAME_TYPED_SPRITES;
        }
        if world_receipt.reflection_pass_rendered {
            flags |= MC_CLASSIC_HD_FRAME_REFLECTIONS;
        }
        if world_receipt.shadow_pass_rendered {
            flags |= MC_CLASSIC_HD_FRAME_SHADOWS;
        }
        let effective_postprocess_mode = match result.receipt.blur_branch {
            ClassicHdBlurBranch::Direct { .. } => 0,
            ClassicHdBlurBranch::Temporal { mode } => mode,
        };
        Ok(McPresentationFrameInfoV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McPresentationFrameInfoV2>(),
            generation,
            tick,
            presentation_revision: query.presentation_revision,
            immutable_input_hash: input_hash,
            width: profile.width as u16,
            height: profile.height as u16,
            stride: profile.stride as u16,
            viewport_width: profile.width as u16,
            index_bytes: u32::try_from(required).expect("Classic HD extent fits u32"),
            provenance_bytes: u32::try_from(required).expect("Classic HD extent fits u32"),
            flags,
            effective_postprocess_mode,
            history_state: u8::from(had_history || result.receipt.prior_main_hash != 0),
            newly_committed: u8::from(result.newly_committed),
            reserved_0: 0,
            index_hash: result.receipt.output_hash,
            provenance_hash: result.receipt.provenance_hash,
            lighting_hash: self.mission.dynamic_lighting_hash_v1(),
            source_arena_hash: self.mission.source_arena_hash_v2(),
            overlay_pixels_written,
            overlay_stage_count,
            reserved_1: 0,
            reserved: [0; 6],
        })
    }

    /// Copy only the owner-authentic HUD overlay into a profile-sized
    /// transparent surface. Unity Enhanced already renders the world through
    /// URP, so asking the Classic HD graph to redraw terrain and typed sprites
    /// here would duplicate visible work without changing the HUD pixels.
    pub(crate) fn classic_hd_owner_overlay_copy(
        &mut self,
        profile_id: u32,
        query: McPresentationFrameQueryV2,
        index_buffer: &mut [u8],
        provenance_buffer: &mut [u8],
    ) -> AbiResult<McPresentationFrameInfoV2> {
        let profile = self.validate_classic_hd_frame_request(
            profile_id,
            &query,
            index_buffer,
            provenance_buffer,
        )?;
        let required = profile.extent();
        index_buffer.fill(0);
        provenance_buffer.fill(0);
        let (overlay_pixels_written, overlay_stage_count, input_hash, newly_committed) = self
            .composite_classic_hd_owner_overlays(
                &profile,
                query,
                index_buffer,
                provenance_buffer,
            )?;
        Ok(McPresentationFrameInfoV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McPresentationFrameInfoV2>(),
            generation: self.generation,
            tick: self.camera_history.tick,
            presentation_revision: query.presentation_revision,
            immutable_input_hash: input_hash,
            width: profile.width as u16,
            height: profile.height as u16,
            stride: profile.stride as u16,
            viewport_width: profile.width as u16,
            index_bytes: u32::try_from(required).expect("Classic HD extent fits u32"),
            provenance_bytes: u32::try_from(required).expect("Classic HD extent fits u32"),
            flags: MC_CLASSIC_HD_FRAME_OWNER_OVERLAY_ONLY
                | MC_PRESENTATION_FRAME_HAS_PROVENANCE
                | MC_PRESENTATION_FRAME_OVERLAYS_COMPOSITED
                | MC_PRESENTATION_FRAME_POSTPROCESS_COMPLETE,
            effective_postprocess_mode: 0,
            history_state: 0,
            newly_committed,
            reserved_0: 0,
            index_hash: byte_hash(index_buffer),
            provenance_hash: byte_hash(provenance_buffer),
            lighting_hash: self.mission.dynamic_lighting_hash_v1(),
            source_arena_hash: self.mission.source_arena_hash_v2(),
            overlay_pixels_written,
            overlay_stage_count,
            reserved_1: 0,
            reserved: [0; 6],
        })
    }

    pub(crate) fn classic_hd_owner_overlay_frame(
        &mut self,
        query: McPresentationFrameQueryV2,
    ) -> AbiResult<sim_core::presentation_frame::PresentationFrameResult> {
        let mut hud_query = query;
        hud_query.viewport_width = LEVEL1_VIEW_WIDTH as u16;
        hud_query.viewport_height = LEVEL1_VIEW_HEIGHT as u16;
        hud_query.screen_stride = LEVEL1_VIEW_WIDTH as u16;
        // The owner HUD is a direct compositor surface. The HD graph owns its
        // separate blur/history decision, so do not blur the source HUD a
        // second time while extracting its marked overlay pixels.
        hud_query.configured_blur_mode = 0;
        hud_query.blur_capability = 1;
        hud_query.high_speed_system = 1;
        hud_query.scratch_present = 1;
        hud_query.vga_resolution = 1;
        hud_query.spatial_smoothing = 0;

        let result = self.presentation_frame(hud_query)?;
        if !result.frame.compositor_complete()
            || result.frame.overlay_pixels_written == 0
            || result.frame.overlay_receipts.is_empty()
        {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "Classic HD owner compositor did not produce a complete HUD overlay",
            ));
        }
        Ok(result)
    }

    /// Composite the owner-authentic fixed-pipeline overlay pixels over the
    /// profile-sized Classic HD world. The HD Rust graph remains authoritative
    /// for terrain and typed sprites; this imports only pixels explicitly
    /// marked as overlays by the existing 320x200 owner compositor, preserving
    /// the original HUD art and layout while fitting it into the profile's
    /// 4:3 surface. The same helper also serves the Enhanced overlay-only
    /// endpoint, where the destination begins transparent instead of carrying
    /// the HD world.
    pub(crate) fn composite_classic_hd_owner_overlays(
        &mut self,
        profile: &ClassicHdRenderProfile,
        query: McPresentationFrameQueryV2,
        indices: &mut [u8],
        provenance: &mut [u8],
    ) -> AbiResult<(u32, u16, u64, u8)> {
        let base_width = LEVEL1_VIEW_WIDTH as u32;
        let base_height = LEVEL1_VIEW_HEIGHT as u32;
        if profile.width < base_width || profile.height < base_height {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!(
                    "Classic HD profile {} cannot fit the fixed owner HUD",
                    profile.id.as_str()
                ),
            ));
        }
        if profile.stride != profile.width {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!(
                    "Classic HD profile {} has an incompatible owner HUD stride",
                    profile.id.as_str()
                ),
            ));
        }

        let owner_result = self.classic_hd_owner_overlay_frame(query)?;
        let owner_frame = owner_result.frame;

        let base_width = usize::try_from(base_width).expect("base width fits usize");
        let base_height = usize::try_from(base_height).expect("base height fits usize");
        let output_width = usize::try_from(profile.width).expect("Classic HD width fits usize");
        let output_height = usize::try_from(profile.height).expect("Classic HD height fits usize");
        let output_stride = usize::try_from(profile.stride).expect("Classic HD stride fits usize");
        let mut written = 0_u32;
        for source_y in 0..base_height {
            for source_x in 0..base_width {
                let source_offset = source_y * base_width + source_x;
                if owner_frame.provenance[source_offset] & OVERLAY_PROVENANCE == 0 {
                    continue;
                }
                let value = owner_frame.indices[source_offset];
                let output_x = source_x * output_width / base_width;
                let output_x_end = (source_x + 1) * output_width / base_width;
                let output_y = source_y * output_height / base_height;
                let output_y_end = (source_y + 1) * output_height / base_height;
                for y in output_y..output_y_end {
                    let row = y * output_stride;
                    for x in output_x..output_x_end {
                        let output_offset = row + x;
                        indices[output_offset] = value;
                        provenance[output_offset] |= OVERLAY_PROVENANCE;
                        written = written
                            .checked_add(1)
                            .expect("Classic HD overlay pixel count fits u32");
                    }
                }
            }
        }

        if written == 0 {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "Classic HD owner compositor marked no pixels for the HUD overlay",
            ));
        }

        Ok((
            written,
            u16::try_from(owner_frame.overlay_receipts.len())
                .expect("Classic HD overlay stage count fits u16"),
            owner_frame.immutable_input_hash,
            u8::from(owner_result.newly_committed),
        ))
    }

    pub(crate) fn world_frame(&mut self) -> AbiResult<Level1TerrainFrame> {
        let ambient_entities = self.mission.ambient_entities().copied().collect::<Vec<_>>();
        let frame = self.render_world_frame(&ambient_entities, &self.renderable_map_chains)?;
        self.commit_world_frame_feedback(&frame)?;
        Ok(frame)
    }

    pub(crate) fn commit_world_frame_feedback(
        &mut self,
        frame: &Level1TerrainFrame,
    ) -> AbiResult<()> {
        let mask = frame.goat_selector_mask;
        self.selector_frames
            .as_mut()
            .expect("world-frame selector clocks were validated")
            .latch_draw_attempts(mask)
            .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error))?;
        self.archer_selector_frames
            .as_mut()
            .expect("world-frame Archer selector clocks were validated")
            .latch_draw_attempts(frame.archer_selector_mask)
            .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error))?;
        self.firefly_selector_frames
            .as_mut()
            .expect("world-frame Firefly selector clocks were validated")
            .latch_draw_attempts(frame.firefly_selector_mask);
        self.villager_selector_frames
            .as_mut()
            .expect("world-frame villager selector clocks were validated")
            .latch_draw_attempts(frame.frozen_actor_selector_mask)
            .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error))?;
        self.fireball_selector_frame
            .as_mut()
            .expect("world-frame Fireball selector clock was validated")
            .latch_draw_attempt(frame.fireball_draw_attempted);
        self.possession_selector_frame
            .as_mut()
            .expect("world-frame Possession selector clock was validated")
            .latch_draw_attempt(frame.possession_draw_attempted);
        self.castle_selector_frame
            .as_mut()
            .expect("world-frame Castle selector clock was validated")
            .latch_draw_attempt(frame.castle_draw_attempted);
        let feedback_tick = self.state.tick;
        let combined_mask = self
            .selector_draw_feedback
            .filter(|(tick, _)| *tick == feedback_tick)
            .map_or(mask, |(_, prior_mask)| prior_mask | mask);
        self.selector_draw_feedback = Some((feedback_tick, combined_mask));
        Ok(())
    }

    pub(crate) fn presentation_frame(
        &mut self,
        query: McPresentationFrameQueryV2,
    ) -> AbiResult<sim_core::presentation_frame::PresentationFrameResult> {
        let assets = Arc::clone(self.terrain_frame_assets.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "presentation frame unavailable without qualified owner cache",
            )
        })?);
        if query.menu_overlay > 6 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                format!(
                    "presentation menu overlay {} is not supported",
                    query.menu_overlay
                ),
            ));
        }
        let hud = level1_hud_snapshot(self);
        let spell_menu = level1_spell_menu_snapshot(self);
        let minimap_entities_abi = level1_minimap_entity_commands(self);
        let minimap_entities = minimap_entities_abi
            .iter()
            .map(|command| Level1MinimapEntityCommand {
                source_slot: command.source_slot,
                x: command.x,
                y: command.y,
                selector: command.selector,
                kind: command.kind,
                palette_index: command.palette_index,
                size: command.size,
                anchor: command.anchor,
                help_selector: command.help_selector,
            })
            .collect::<Vec<_>>();
        let map_entities = level1_map_entity_commands(self);
        let spell_slot = |slot: McLevel1HudSpellSlotV2| LiveSpellSlot {
            holder_entity_index: slot.holder_entity_index,
            model: slot.model,
            tier: slot.tier,
            enabled_flags: slot.enabled_flags,
            active_value: slot.active_value,
            max_mana: slot.max_mana,
            mana_regen: slot.mana_regen,
            caster_mana: slot.caster_mana,
            frame_selector: slot.frame_selector,
            icon_selector: slot.icon_selector,
        };
        let fire = self.context.spell_fire.as_ref();
        let mut family_available = std::array::from_fn(|family| {
            u8::from(
                assets
                    .indexed_overlays
                    .spell_menu_profile(family)
                    .expect("qualified cache has all 26 spell families")
                    .family_header_tier_count
                    != 0,
            )
        });
        // DrawBottomSpellsMenu suppresses the cave-only family in the outdoor
        // Level 1 profile even though the source descriptor itself is live.
        family_available[25] = 0;
        let holder_entity_index = fire.map_or([0_u16; 26], |fire| fire.enabled_holder_indices);
        let family_unlocked_tier = fire.map_or([0_i8; 26], |fire| fire.spell_levels);
        let family_selected_tier = fire.map_or([0_i8; 26], |fire| fire.family_selected_tiers);
        let family_hand_eligibility = fire.map_or([0_u8; 26], |fire| fire.spell_hand_eligibility);
        let family_experience = fire.map_or([0_i32; 26], |fire| fire.spell_experience);
        let family_enabled_flags = std::array::from_fn(|family| {
            assets
                .indexed_overlays
                .spell_menu_profile(family)
                .expect("qualified cache has all 26 spell families")
                .family_flags
        });
        let family_active_value = std::array::from_fn(|family| {
            let holder = holder_entity_index[family];
            fire.and_then(|fire| {
                fire.holders
                    .iter()
                    .find(|candidate| candidate.entity_index == holder)
            })
            .map_or(0, |holder| holder.active_value)
        });
        let family_mana_cost = std::array::from_fn(|family| {
            assets
                .indexed_overlays
                .spell_menu_profile(family)
                .expect("qualified cache has all 26 spell families")
                .mana_cost
        });
        let family_max_mana_limit = std::array::from_fn(|family| {
            assets
                .indexed_overlays
                .spell_menu_profile(family)
                .expect("qualified cache has all 26 spell families")
                .max_mana_limit
        });
        let debug_source = &self.world.entities()[usize::from(self.state.entity_index)];
        let debug_record = debug_source.record_bytes();
        let read_i16 =
            |offset: usize| i16::from_le_bytes([debug_record[offset], debug_record[offset + 1]]);
        let read_u16 =
            |offset: usize| u16::from_le_bytes([debug_record[offset], debug_record[offset + 1]]);
        let read_i32 = |offset: usize| {
            i32::from_le_bytes([
                debug_record[offset],
                debug_record[offset + 1],
                debug_record[offset + 2],
                debug_record[offset + 3],
            ])
        };
        let map_mode = matches!(self.mission.player_menu_state, 6 | 7 | 8 | 11 | 12 | 14);
        let bridge = build_level1_final_main(
            &assets.indexed_overlays,
            Level1PresentationLive {
                flight: self.state,
                menu_state: self.mission.player_menu_state,
                player_life: self.mission.player_life,
                player_immobilize_ticks: self.state.immobilize_counter.cast_unsigned().into(),
                terrain: self.terrain.planes(),
                hud: LiveHud {
                    mana_color_index: hud.mana_color_index,
                    mana_max_color_index: hud.mana_max_color_index,
                    life_color_index: hud.life_color_index,
                    color_phase_1: hud.color_phase_1,
                    color_phase_2: hud.color_phase_2,
                    player_hit_frame_ticks: hud.player_hit_frame_ticks,
                    castle_hit_frame_ticks: hud.castle_hit_frame_ticks,
                    global_mana_denominator: hud.global_mana_denominator,
                    secured_mana_component: hud.secured_mana_component,
                    goal_percent: hud.goal_percent,
                    player_life: self.mission.player_life,
                    player_max_life: self.mission.player_max_life,
                    player_mana: self.mission.player_mana,
                    player_max_mana: self.mission.player_max_mana,
                    castle_active: self.mission.castle_exists,
                    castle_level: self.mission.castle_level,
                    castle_life: self.mission.castle_life,
                    castle_max_life: self.mission.castle_max_life,
                    castle_mana: self.mission.castle_mana,
                    castle_mana_capacity: self.mission.castle_mana_capacity,
                    player_status_flags: hud.player_status_flags,
                    player_status_word: u16::from(self.mission.exit_player_status_flags),
                    balloon_level: 0,
                    balloon_life: [0; 3],
                    balloon_max_life: [0; 3],
                    balloon_mana: [0; 3],
                    balloon_max_mana: [0; 3],
                    left: spell_slot(hud.left),
                    right: spell_slot(hud.right),
                },
                spell_menu: LiveSpellMenu {
                    selected_ui_index: spell_menu.selected_ui_index,
                    selected_tier: spell_menu.selected_tier,
                    left_family: spell_menu.left_family,
                    right_family: spell_menu.right_family,
                    family_available,
                    holder_entity_index,
                    family_unlocked_tier,
                    family_selected_tier,
                    family_hand_eligibility,
                    family_enabled_flags,
                    family_active_value,
                    family_experience,
                    family_mana_cost,
                    family_max_mana_limit,
                    castle_mana: self.mission.castle_mana,
                    caster_mana: self.mission.player_mana,
                    player_colour_0: hud.mana_color_index,
                    player_colour_1: hud.mana_max_color_index,
                    active_suppression_phase: hud.color_phase_1 != 0,
                    cursor_visible: self.presentation_ui.cursor_visible
                        && matches!(self.presentation_ui.joystick, 4 | 5 | 6 | 8 | 11 | 12 | 13),
                    cursor_x: self.presentation_ui.mouse_x / 2,
                    cursor_y: self.presentation_ui.mouse_y / 2,
                },
                menu: self.presentation_ui,
                debug_entity: LiveDebugEntity {
                    pointer_value: u32::from_le_bytes([
                        debug_record[0],
                        debug_record[1],
                        debug_record[2],
                        debug_record[3],
                    ]),
                    source_slot: debug_source.index,
                    class: debug_source.class,
                    model: debug_source.model,
                    action: debug_source.action,
                    life: debug_source.life,
                    max_life: debug_source.max_life,
                    actual_speed: debug_source.actual_speed,
                    min_speed: read_i16(132),
                    max_speed: read_i16(134),
                    position: self.state.position,
                    source_id: debug_source.source_id,
                    parent_id: read_u16(40),
                    mana: debug_source.mana,
                    max_mana: read_i32(140),
                },
                minimap_entities: &minimap_entities,
                minimap_marks: self.mission.minimap_marks(&self.state),
                map_entities: &map_entities,
                map_marks: self.mission.map_marks(&self.state),
                objective_selector: self.mission.current_text_selector,
                objective_ticks: u16::from(self.mission.objective_textbox_ticks),
                level_completed: self.mission.level_completed,
                exit_sequence_active: self.mission.exit_sequence_active,
                exit_display_blank: self.mission.exit_display_blank,
                exit_blur_mode: self.mission.exit_blur_mode,
                notification_active: self.mission.player_notification_active,
                notification_id: self.mission.player_notification_id,
                notification_state: self.mission.player_notification_state,
                notification_timer: self.mission.player_notification_timer,
                notification_player_name: self.presentation_ui.notification_player_name,
                level_number: 1,
                player_turn: self.source_turn,
                sound_number: self.sound_number,
                game_timer_turn: self.game_timer_turn,
                debug_fps: self.debug_fps,
                active_entity_count: u16::try_from(
                    self.mission
                        .source_slots()
                        .iter()
                        .filter(|slot| slot.active)
                        .count(),
                )
                .expect("source arena count fits u16"),
                heap_size: self.debug_heap_size,
                memory_used: self.debug_memory_used,
                memory_free: self.debug_memory_free,
            },
            PresentationBridgeQuery {
                minimap_enabled: query.minimap_enabled != 0,
                top_bar_enabled: query.top_bar_enabled != 0,
                menu_overlay: query.menu_overlay,
                game_paused: query.game_paused != 0,
                level_end: query.level_end != 0,
                objective_selector: (query.objective_selector != u8::MAX)
                    .then_some(query.objective_selector),
                objective_textbox: query.objective_textbox != 0,
                tutorial_or_briefing: query.tutorial_or_briefing != 0,
                help_popup: query.help_popup != 0,
                debug_flags: query.debug_text,
            },
        )
        .map_err(|error| AbiError::new(MC_STATUS_GENERATION_MISMATCH, error.0))?;
        let current_lighting_receipt = self
            .mission
            .dynamic_lighting_execution_receipt(&self.terrain.planes().shading)
            .map_err(|error| AbiError::new(MC_STATUS_GENERATION_MISMATCH, error))?;
        if current_lighting_receipt != self.lighting_receipt {
            return Err(AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "dynamic-light state/shading changed without an executed scheduler receipt",
            ));
        }
        let source_render_inputs = self.source_render_inputs(map_mode)?;
        let notification_feedback = bridge.notification_feedback;
        let input = PresentationFrameInput {
            generation: query.generation,
            tick: query.tick,
            presentation_revision: query.presentation_revision,
            state_hash: state_hash(&self.state),
            source_arena_hash: self.mission.source_arena_hash_v2(),
            source_render_input_hash: self.source_render_input_hash_for(&source_render_inputs)?,
            lighting_hash: self.mission.dynamic_lighting_hash_v1(),
            camera_hash: camera_history_hash(self.camera_history),
            source_reflections_expected: i32::from(self.camera_history.z)
                < sim_core::terrain_reflection::REFLECTION_EYE_Z_LIMIT,
            postprocess: IndexedPostprocessInput {
                configured_mode: if self.mission.exit_sequence_active {
                    self.mission.exit_blur_mode
                } else {
                    query.configured_blur_mode
                },
                blur_capability: query.blur_capability != 0,
                high_speed_system: query.high_speed_system != 0,
                scratch_present: query.scratch_present != 0,
                vga_resolution: query.vga_resolution,
                menu_state: self.mission.player_menu_state,
                viewport_width: query.viewport_width,
                viewport_height: query.viewport_height,
                screen_stride: query.screen_stride,
                actual_speed: self.state.actual_speed,
                spatial_smoothing: query.spatial_smoothing != 0,
            },
            overlay: bridge.overlay,
            lighting_receipt: Some(self.lighting_receipt),
        };
        let mut goat_candidate = self.selector_frames.clone().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "presentation source renderer has no Goat selector clocks",
            )
        })?;
        let mut archer_candidate = self.archer_selector_frames.clone().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "presentation source renderer has no Archer selector clocks",
            )
        })?;
        let mut firefly_candidate = self.firefly_selector_frames.clone().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "presentation source renderer has no Firefly selector clocks",
            )
        })?;
        let mut villager_candidate = self.villager_selector_frames.clone().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "presentation source renderer has no villager selector clocks",
            )
        })?;
        let mut fireball_candidate = self.fireball_selector_frame.clone().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "presentation source renderer has no Fireball selector clock",
            )
        })?;
        let mut possession_candidate = self.possession_selector_frame.clone().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "presentation source renderer has no Possession selector clock",
            )
        })?;
        let mut castle_candidate = self.castle_selector_frame.clone().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "presentation source renderer has no Castle selector clock",
            )
        })?;
        let mut presentation =
            std::mem::replace(&mut self.presentation_frame, PresentationFrameState::new(0));
        let mut feedback_candidate = None;
        let result =
            presentation.finalize(input, &assets.tablesn, &assets.indexed_overlays, || {
                let frame = self
                    .render_source_world_frame(&source_render_inputs)
                    .map_err(|error| PresentationFrameError(error.message))?;
                let receipt = frame
                    .source_execution_receipt()
                    .map_err(|error| PresentationFrameError(error.0))?;
                goat_candidate
                    .latch_draw_attempts(frame.goat_selector_mask)
                    .map_err(PresentationFrameError)?;
                archer_candidate
                    .latch_draw_attempts(frame.archer_selector_mask)
                    .map_err(PresentationFrameError)?;
                firefly_candidate.latch_draw_attempts(frame.firefly_selector_mask);
                villager_candidate
                    .latch_draw_attempts(frame.frozen_actor_selector_mask)
                    .map_err(PresentationFrameError)?;
                fireball_candidate.latch_draw_attempt(frame.fireball_draw_attempted);
                possession_candidate.latch_draw_attempt(frame.possession_draw_attempted);
                castle_candidate.latch_draw_attempt(frame.castle_draw_attempted);
                feedback_candidate = Some((
                    goat_candidate,
                    archer_candidate,
                    firefly_candidate,
                    villager_candidate,
                    fireball_candidate,
                    possession_candidate,
                    castle_candidate,
                    frame.goat_selector_mask,
                ));
                Ok(SourceRenderedFrame {
                    map_world: map_mode.then(|| frame.indices.clone()),
                    indices: frame.indices,
                    receipt,
                })
            });
        self.presentation_frame = presentation;
        let result =
            result.map_err(|error| AbiError::new(MC_STATUS_GENERATION_MISMATCH, error.0))?;
        if result.newly_committed {
            let (goat, archer, firefly, villager, fireball, possession, castle, goat_mask) =
                feedback_candidate
                    .expect("newly committed presentation owns validated clock candidates");
            self.selector_frames = Some(goat);
            self.archer_selector_frames = Some(archer);
            self.firefly_selector_frames = Some(firefly);
            self.villager_selector_frames = Some(villager);
            self.fireball_selector_frame = Some(fireball);
            self.possession_selector_frame = Some(possession);
            self.castle_selector_frame = Some(castle);
            self.selector_draw_feedback = Some((self.state.tick, goat_mask));
            if let Some((active, state, timer)) = notification_feedback {
                self.notification_draw_feedback = Some((self.state.tick, active, state, timer));
            }
        }
        Ok(result)
    }

    pub(crate) fn selector_frame_batch(
        &self,
        start_ordinal: u32,
        written: u32,
    ) -> AbiResult<McSelectorFrameBatchV2> {
        let frames = self.selector_frames.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "selector-frame state unavailable: MC_ASSET_CACHE_PATH was not supplied when the session opened",
            )
        })?;
        let (last_draw_tick, last_draw_mask, has_draw_feedback) = self
            .selector_draw_feedback
            .map_or((0, 0, 0), |(tick, mask)| (tick, u64::from(mask), 1));
        let written_usize = usize::try_from(written)
            .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "selector written count exceeds usize"))?;
        let start_usize = usize::try_from(start_ordinal)
            .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "selector start exceeds usize"))?;
        Ok(McSelectorFrameBatchV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McSelectorFrameBatchV2>(),
            generation: self.generation,
            tick: self.state.tick,
            selector_state_hash: selector_state_hash_v1(frames, self.selector_draw_feedback),
            last_draw_tick,
            last_draw_mask,
            valid_draw_mask: u64::from(GOAT_TMAP_DRAW_MASK),
            start_ordinal,
            written,
            total_count: GOAT_TMAP_SELECTORS.len() as u32,
            has_more: u8::from(start_usize + written_usize < GOAT_TMAP_SELECTORS.len()),
            has_draw_feedback,
            reserved_0: 0,
            reserved: [0; 4],
        })
    }

    pub(crate) fn evaluate_goat_draw_admission(&mut self) -> AbiResult<McSelectorFrameBatchV2> {
        if self.world.goat_tmap_bank().is_none() {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "Goat CPU draw admission unavailable without qualified owner-cache descriptors",
            ));
        }
        let camera = self.camera_history;
        let slope = self.camera_slope_sample;
        let planes = self.terrain.planes();
        let render_x = camera.x.wrapping_add(slope.offset_x.cast_unsigned());
        let render_y = camera.y.wrapping_add(slope.offset_y.cast_unsigned());
        let lattice = outdoor_grid(OutdoorGridInput {
            render_x,
            render_y,
            yaw: camera.yaw,
            turn: camera.source_turn,
            shading: &planes.shading,
            angles: &planes.angles,
        });
        let visited = outdoor_billboard_visit_map(&lattice, camera.yaw);
        let admission = evaluate_goat_draw_admission(
            self.live_world.goats(),
            GoatDrawCamera {
                render_x,
                render_y,
                yaw: camera.yaw,
            },
            &visited,
            true,
        );
        let mask = admission.selector_mask;
        let frames = self.selector_frames.as_mut().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "selector-frame state unavailable without qualified owner cache",
            )
        })?;
        frames
            .latch_draw_attempts(mask)
            .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error))?;
        let combined_mask = self
            .selector_draw_feedback
            .filter(|(tick, _)| *tick == self.state.tick)
            .map_or(mask, |(_, prior_mask)| prior_mask | mask);
        self.selector_draw_feedback = Some((self.state.tick, combined_mask));
        self.goat_draw_admission = Some(admission);
        let mut receipt = self.selector_frame_batch(0, 0)?;
        receipt.has_more = 0;
        Ok(receipt)
    }

    pub(crate) fn snapshot(&self) -> McSessionSnapshotV2 {
        let player = &self.world.players()[usize::from(self.world.runtime().local_player_slot)];
        let fire = self.context.spell_fire.as_ref();
        let selected = fire.map_or(0, |fire| fire.selected_spell_ui_index);
        let left_holder = fire.map_or(0, |fire| fire.left_holder_index);
        let right_holder = fire.map_or(0, |fire| fire.right_holder_index);
        let selected_holder = fire.map_or(0, |fire| fire.selected_holder_index);
        let holder_model = |holder_index| {
            fire.and_then(|fire| {
                fire.holders
                    .iter()
                    .find(|holder| holder.entity_index == holder_index)
                    .map(|holder| holder.model)
            })
        };
        let left_spell_slot = holder_model(left_holder).unwrap_or(player.left_spell_slot);
        let right_spell_slot = holder_model(right_holder).unwrap_or(player.right_spell_slot);
        let caster_mana = fire.map_or(0, |fire| fire.caster.mana);
        McSessionSnapshotV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McSessionSnapshotV2>(),
            generation: self.generation,
            tick: self.state.tick,
            event_head: self.event_head,
            state_hash: state_hash(&self.state),
            conformance_hash: conformance_hash_v2(&self.state, &self.context),
            entity_index: self.state.entity_index,
            x: self.state.position.x,
            y: self.state.position.y,
            z: self.state.position.z,
            yaw: self.state.yaw,
            display_pitch: self.state.display_pitch,
            effective_pitch: self.state.effective_pitch,
            roll_accumulator: self.state.roll_accumulator,
            target_speed: self.state.target_speed,
            actual_speed: self.state.actual_speed,
            strafe_speed: self.state.strafe_speed,
            phase: self.state.entity_phase,
            last_resolution: self.last_resolution,
            profile: profile_code(self.world.profile().cave_mode),
            local_player_slot: self.world.runtime().local_player_slot,
            left_spell_slot,
            right_spell_slot,
            selected_spell_ui_index: selected,
            objective_textbox_ticks: self.world.runtime().objective_textbox_ticks,
            objective_count: u8::try_from(self.world.runtime().objectives.len())
                .expect("validated objective count fits u8"),
            objectives_authority: MC_AUTHORITY_FROZEN_OBSERVED,
            reserved_1: [0; 2],
            left_spell_holder: left_holder,
            right_spell_holder: right_holder,
            selected_spell_holder: selected_holder,
            reserved_0: 0,
            caster_mana,
            maptype_music: self.world.runtime().maptype_music,
            fire_sequence: self.fire_sequence,
            reserved: [0; 4],
        }
    }

    #[cfg(test)]
    pub(crate) fn event_capacity(&self) -> usize {
        self.event_capacity
    }

    #[cfg(not(test))]
    pub(crate) fn event_capacity(&self) -> usize {
        EVENT_CAPACITY
    }

    pub(crate) fn append_event(&mut self, event: McEventV2) {
        if self.events.len() == self.event_capacity()
            && let Some(expired) = self.events.pop_front()
        {
            // Audio sidecars have an explicit client release lifecycle. They
            // may outlive the bounded event journal while paused or while a
            // client drains a multi-step backlog.
            let _ = expired;
        }
        self.events.push_back(event);
    }

    pub(crate) fn next_event_base(&mut self, kind: u32, state_hash: u64) -> McEventV2 {
        self.event_head = next_nonzero(self.event_head);
        McEventV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McEventV2>(),
            generation: self.generation,
            sequence: self.event_head,
            state_hash,
            kind,
            code: 0,
            entity_index: 0,
            player_index: -1,
            value_0: 0,
            value_1: 0,
            asset_ref_index: MC_ASSET_REF_NONE,
            reserved: [0; 4],
        }
    }

    pub(crate) fn apply_castle_terrain_updates(&mut self) {
        let updates = self
            .mission
            .drain_castle_terrain_updates()
            .collect::<Vec<_>>();
        if updates.is_empty() {
            return;
        }
        for update in updates {
            if update.is_ambient() {
                self.terrain
                    .apply_ambient_update(update)
                    .expect("qualified Level 1 ambient terrain update");
            } else {
                let footprint = self
                    .mission
                    .castle_stage_one_footprint()
                    .expect("Castle terrain updates require the validated BUILD[1] footprint");
                self.terrain
                    .apply_castle_update(update, footprint)
                    .expect("qualified Level 1 Castle terrain update");
            }
        }
    }

    pub(crate) fn step(&mut self, command: PlayerCommand) {
        #[cfg(test)]
        self.immediate_released_holder_handoff.clear();
        if self.game_paused {
            self.mission.begin_paused_outer_presentation();
            // `UpdateEntities_57730` stores its global-RNG draw before testing
            // GAME_PAUSED. Preserve that source-visible prefix while keeping
            // every post-gate entity action and unrelated turn clock frozen.
            self.mission.advance_paused_update_rng();
            self.mission.finish_paused_outer_presentation(&self.state);
            let state_hash = state_hash(&self.state);
            for event in self.mission.events().to_vec() {
                let record = self.encode_mission_event(event, state_hash);
                self.append_event(record);
            }
            // `PlayEntitySounds_6E150` is gated while the selected
            // single-player/no-CD profile is paused (`byte_38591 == 0`).
            // Keep prepared rows unresolved so the first unpaused flush
            // coalesces them with that frame's rows using the source pool.
            return;
        }
        if let Some((_, active, state, timer)) = self.notification_draw_feedback.take() {
            self.mission.player_notification_active = active;
            self.mission.player_notification_state = state;
            self.mission.player_notification_timer = timer;
            if !active {
                self.mission.player_notification_id = 0;
            }
        }
        self.mission.finish_hud_draw(self.state.tick);
        if let (Some(frames), Some(bank)) =
            (self.selector_frames.as_mut(), self.world.goat_tmap_bank())
        {
            frames.service_unpaused_outer_turn(bank);
        }
        if let (Some(frames), Some(assets)) = (
            self.archer_selector_frames.as_mut(),
            self.terrain_frame_assets.as_ref(),
        ) {
            frames.service_unpaused_outer_turn(&assets.archer_tmap_bank);
        }
        if let (Some(frames), Some(assets)) = (
            self.firefly_selector_frames.as_mut(),
            self.terrain_frame_assets.as_ref(),
        ) {
            frames.service_unpaused_outer_turn(&assets.firefly_tmap_bank);
        }
        if let (Some(frames), Some(assets)) = (
            self.villager_selector_frames.as_mut(),
            self.terrain_frame_assets.as_ref(),
        ) {
            // remc2's C++ transcription omits the clear-store at the common
            // texture clock service. We retain the already accepted
            // executable-order clear-before-advance policy used by Goat and
            // Archer clocks.
            frames.service_unpaused_outer_turn(&assets.frozen_actor_tmap_bank);
        }
        if let (Some(frame), Some(assets)) = (
            self.fireball_selector_frame.as_mut(),
            self.terrain_frame_assets.as_ref(),
        ) {
            frame.service_unpaused_outer_turn(&assets.combat_tmap_bank);
        }
        if let (Some(frame), Some(assets)) = (
            self.possession_selector_frame.as_mut(),
            self.terrain_frame_assets.as_ref(),
        ) {
            frame.service_unpaused_outer_turn(&assets.combat_tmap_bank);
        }
        if let (Some(frame), Some(assets)) = (
            self.castle_selector_frame.as_mut(),
            self.terrain_frame_assets.as_ref(),
        ) {
            frame.service_unpaused_outer_turn(&assets.economy_tmap_bank);
        }
        self.mission.advance_exit_palette_frame();
        self.source_turn = self.source_turn.wrapping_add(1);
        self.game_timer_turn = self.game_timer_turn.wrapping_add(1);
        if !self.mission.exit_sequence_owns_player_action()
            && self.mission.player_action == sim_core::level1_mission::PLAYER_ACTION_ACTIVE
        {
            if command.opcode == 8 {
                self.camera_fov = self.camera_fov.wrapping_add(i16::from(command.argument_1));
            }
            self.camera_look_yaw = command.argument_6.cast_signed();
            self.camera_look_pitch = command.argument_8.cast_signed();
        }
        // `PlayerEvents_51BB0` stores this camera record after applying the
        // input/view fields but before `UpdateEntities_57730` advances flight.
        self.camera_history = CameraHistory::capture(
            &self.state,
            self.source_turn,
            self.camera_fov,
            self.camera_look_yaw,
            self.camera_look_pitch,
        );
        self.camera_slope_sample = self.camera_slope.update(
            self.terrain.ground_heights(),
            self.camera_history.x,
            self.camera_history.y,
        );
        self.mission.sync_collected_spell_holders(&mut self.context);
        self.mission
            .sync_speed_holder_projection(&mut self.state, &mut self.context);
        let holders_before = self
            .context
            .spell_fire
            .as_ref()
            .map_or_else(Vec::new, |fire| fire.holders.clone());
        self.context.events.clear();
        let world = Arc::clone(&self.world);
        let player_control_flags = world
            .entities()
            .get(usize::from(self.state.entity_index))
            .map_or(0, |entity| entity.control_flags);
        let initial_target = [GoatPlayerTarget {
            entity_index: self.state.entity_index,
            position: self.state.position,
            control_flags: player_control_flags,
        }];
        self.mission.set_external_collision_targets(
            self.live_world
                .goats()
                .iter()
                .map(goat_collision_target)
                .chain(
                    self.live_world
                        .villagers()
                        .iter()
                        .map(villager_collision_target),
                )
                .chain(self.live_world.houses().iter().map(house_collision_target)),
        );
        {
            let (_, shading) = self.terrain.dynamic_lighting_planes();
            self.mission
                .subtract_dynamic_lighting_before_updates(shading);
        }
        self.mission.begin_source_turn_at(self.state.position);
        let mut goat_events = self
            .reclaim_imported_source_slots()
            .into_iter()
            .map(|event| (event, self.mission.global_rng()))
            .collect::<Vec<_>>();
        if self.mission.take_group_7_villager_release() {
            let released = self.live_world.release_villager_stage_group_7();
            debug_assert_eq!(released, 28);
        }
        let begin_goat_events = self.live_world.begin_source_turn(&initial_target);
        self.apply_goat_event_side_effects(world.as_ref(), &begin_goat_events);
        goat_events.extend(
            begin_goat_events
                .into_iter()
                .map(|event| (event, self.mission.global_rng())),
        );
        let mut source_views =
            level1_turn_start_source_views(&self.mission, &self.state, &self.live_world);
        let frozen_house_source_views = source_views.clone();
        let mut evidence = Vec::new();
        let mut released_holder_handoff = false;
        for source_slot in 1_u16..1_000 {
            if let Some((owner_entity_index, payload)) =
                self.mission.take_building_possession_mailbox(source_slot)
            {
                self.live_world
                    .submit_house_possession(source_slot, owner_entity_index, payload)
                    .expect("possession mailbox targets a live eligible house");
            }
            let action_51_blocked = self
                .mission
                .current_house_action_51_blocked(&frozen_house_source_views, source_slot);
            let house_events = self.live_world.step_house_source_slot_with_action_51_block(
                source_slot,
                &self.terrain,
                action_51_blocked,
            );
            self.apply_house_event_side_effects(&house_events, &frozen_house_source_views);
            let player_target = [GoatPlayerTarget {
                entity_index: self.state.entity_index,
                position: self.state.position,
                control_flags: player_control_flags,
            }];
            let slot_goat_events =
                self.live_world
                    .step_source_slot(source_slot, &self.terrain, &player_target);
            let villager_effects = self.live_world.step_villager_source_slot(
                source_slot,
                &self.terrain,
                &source_views,
            );
            self.mission
                .apply_imported_villager_effects(&villager_effects);
            self.apply_goat_event_side_effects(world.as_ref(), &slot_goat_events);
            goat_events.extend(
                slot_goat_events
                    .into_iter()
                    .map(|event| (event, self.mission.global_rng())),
            );
            if let Some(house) = self
                .live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == source_slot)
            {
                self.mission
                    .update_external_collision_target(house_collision_target(house));
                self.mission.sync_imported_source_slot(
                    house.entity_index,
                    house.active,
                    house.action,
                    house.phase,
                    house.rng,
                    house.control_flags,
                );
            } else if let Some(goat) = self
                .live_world
                .goats()
                .iter()
                .find(|goat| goat.entity_index == source_slot)
            {
                self.mission
                    .update_external_collision_target(goat_collision_target(goat));
                self.mission.sync_imported_source_slot(
                    goat.entity_index,
                    goat.active,
                    goat.action,
                    goat.phase,
                    goat.rng,
                    goat.control_flags,
                );
            } else if let Some(villager) = self
                .live_world
                .villagers()
                .iter()
                .find(|villager| villager.entity_index == source_slot)
            {
                self.mission
                    .update_external_collision_target(villager_collision_target(villager));
                self.mission.sync_imported_source_slot(
                    villager.entity_index,
                    villager.active,
                    villager.action,
                    villager.phase,
                    villager.rng,
                    villager.control_flags,
                );
            }
            refresh_level1_current_source_view(&mut source_views, &self.live_world, source_slot);
            if let Some(dolmen) = world
                .entities()
                .get(usize::from(source_slot))
                .filter(|entity| {
                    entity.active && entity.class == 2 && entity.model == 2 && entity.action == 6
                })
            {
                self.mission.step_dolmen_source_slot(
                    &self.state,
                    source_slot,
                    dolmen.collision_target(),
                );
            }

            let (fireball_activated, possession_activated, castle_cast_yaw) = if source_slot
                == self.state.entity_index
            {
                self.mission
                    .prepare_player_action_before_flight(&mut self.state);
                evidence = if self.mission.exit_sequence_owns_player_action() {
                    self.state.tick = self.state.tick.wrapping_add(1);
                    Vec::new()
                } else {
                    match self.mission.player_action {
                        sim_core::level1_mission::PLAYER_ACTION_ACTIVE => {
                            step_outer_turn_with_context(
                                &mut self.state,
                                command,
                                0,
                                world.profile(),
                                &self.terrain,
                                &mut self.context,
                            )
                        }
                        sim_core::level1_mission::PLAYER_ACTION_DYING => {
                            vec![step_entity_body_with_context(
                                &mut self.state,
                                world.profile(),
                                &self.terrain,
                                &mut self.context,
                            )]
                        }
                        _ => Vec::new(),
                    }
                };
                if !self.mission.exit_sequence_owns_player_action() {
                    let flight_proposed = evidence
                        .last()
                        .map_or(self.state.position, |pass| pass.proposed);
                    let lifecycle_action = self.mission.player_action;
                    self.mission
                        .advance_player_lifecycle_after_flight_with_proposed(
                            &self.terrain,
                            &mut self.state,
                            &mut self.context,
                            flight_proposed,
                        );
                    let released_holder_this_pass = lifecycle_action
                        == sim_core::level1_mission::PLAYER_ACTION_DYING
                        && self.mission.player_lifecycle_trace().iter().any(|entry| {
                            entry.step
                                == sim_core::level1_mission::PLAYER_LIFECYCLE_TRACE_HOLDER_FAMILY
                        });
                    released_holder_handoff |= released_holder_this_pass;
                    if released_holder_this_pass {
                        for released in self.mission.released_spell_holders().iter().flatten() {
                            let tile = (
                                released.position.x.to_be_bytes()[0],
                                released.position.y.to_be_bytes()[0],
                            );
                            self.renderable_map_chains
                                .sync_entity(RenderableMapSeed {
                                    entity_index: released.source_entity_index,
                                    active: true,
                                    x: released.position.x,
                                    y: released.position.y,
                                })
                                .expect("qualified immediate spell holder map insertion");
                            debug_assert!(
                                self.renderable_map_chains
                                    .entity_chain(tile.0, tile.1)
                                    .contains(&released.source_entity_index),
                                "released holder must be visible before later source slots dispatch"
                            );
                            #[cfg(test)]
                            self.immediate_released_holder_handoff.push((
                                released.family,
                                released.source_entity_index,
                                tile.0,
                                tile.1,
                            ));
                        }
                    }
                    if let Some(marker) = self
                        .mission
                        .player_death_marker()
                        .filter(|marker| marker.active)
                    {
                        self.renderable_map_chains
                            .sync_entity(RenderableMapSeed {
                                entity_index: marker.source_entity_index,
                                active: true,
                                x: marker.position.x,
                                y: marker.position.y,
                            })
                            .expect("qualified player death marker map insertion");
                    }
                    for burst in self
                        .mission
                        .death_bursts()
                        .iter()
                        .filter(|burst| burst.active)
                    {
                        self.renderable_map_chains
                            .sync_entity(RenderableMapSeed {
                                entity_index: burst.source_entity_index,
                                active: true,
                                x: burst.position.x,
                                y: burst.position.y,
                            })
                            .expect("qualified death burst map insertion");
                    }
                }
                // A multi-pass outer turn clears `accepted_models` when each
                // pass latches the command. Detect the source-visible holder
                // transition instead, matching the direct Rust session.
                let activated_model = |model| {
                    self.context.spell_fire.as_ref().is_some_and(|fire| {
                        fire.holders.iter().any(|holder| {
                            holder.model == model
                                && holder.activation_value != 0
                                && holder.active_value == holder.activation_value.cast_signed()
                                && holders_before
                                    .iter()
                                    .find(|before| before.entity_index == holder.entity_index)
                                    .is_none_or(|before| before.active_value != holder.active_value)
                        })
                    })
                };
                let fireball = activated_model(0);
                let possession = activated_model(1);
                let castle = activated_model(CASTLE_SPELL_FAMILY);
                self.mission
                    .capture_spell_holder_active_values(&self.context);
                (
                    fireball,
                    possession,
                    castle.then_some(self.state.yaw.wrapping_add(self.camera_look_yaw) & 0x07ff),
                )
            } else {
                (false, false, None)
            };
            // remc2 updates a moving entity's terrain-cell link during that
            // entity's source action. Goats/villagers have already acted
            // above, so expose their current chain positions before the Rust
            // mission dispatches the same source slot. Mission-owned rows are
            // synchronized again after their action below.
            self.sync_source_slot_renderable_records(source_slot)
                .expect("validated pre-dispatch source map update");
            let mission_event_start = self.mission.events().len();
            if !self.mission.exit_sequence_owns_player_action()
                && self.mission.player_action == sim_core::level1_mission::PLAYER_ACTION_ACTIVE
            {
                self.mission.dispatch_speed_holder_slot(
                    source_slot,
                    &mut self.state,
                    &mut self.context,
                );
            }
            let source_effects = self.mission.dispatch_source_slot_with_views_and_look(
                &self.terrain,
                &self.state,
                &source_views,
                source_slot,
                fireball_activated,
                possession_activated,
                castle_cast_yaw,
                self.camera_look_yaw,
                self.camera_look_pitch,
            );
            self.mission
                .apply_pending_player_transients(&mut self.state);
            for seed in apply_level1_source_effects(
                &mut self.mission,
                &mut self.live_world,
                &source_effects,
            )
            .expect("resident source effects apply after slot dispatch")
            {
                self.renderable_map_chains
                    .sync_entity(seed)
                    .expect("resident source effect updates presentation map chain");
            }
            if source_slot == self.state.entity_index && self.mission.exit_sequence_active {
                self.mission.advance_exit_sequence(
                    &self.terrain,
                    &mut self.state,
                    &mut self.context,
                    &mut self.camera_fov,
                );
            }
            self.apply_castle_terrain_updates();
            self.inject_new_fireball_impacts(mission_event_start);
            self.sync_source_slot_renderable_records(source_slot)
                .expect("validated source renderable map update");
            goat_events.extend(
                self.reclaim_imported_source_slots()
                    .into_iter()
                    .map(|event| (event, self.mission.global_rng())),
            );
        }
        self.live_world.finish_source_turn();
        {
            let (ground_heights, shading) = self.terrain.dynamic_lighting_planes();
            self.mission.service_dynamic_lighting_after_updates(
                &self.state,
                &source_views,
                ground_heights,
                shading,
            );
            self.lighting_receipt = self
                .mission
                .dynamic_lighting_execution_receipt(shading)
                .expect("qualified C ABI dynamic-light execution has fixed shading extent");
            Arc::make_mut(&mut self.classic_hd_shading).copy_from_slice(shading);
        }
        self.mission.finish_source_turn(&self.state);
        self.sync_supported_renderable_records()
            .expect("validated final source renderable map update");
        if released_holder_handoff {
            for released in self.mission.released_spell_holders().iter().flatten() {
                self.renderable_map_chains
                    .sync_entity(RenderableMapSeed {
                        entity_index: released.source_entity_index,
                        active: true,
                        x: released.position.x,
                        y: released.position.y,
                    })
                    .expect("qualified released spell holder map insertion");
                debug_assert!(
                    self.renderable_map_chains
                        .entity_chain(
                            released.position.x.to_be_bytes()[0],
                            released.position.y.to_be_bytes()[0],
                        )
                        .contains(&released.source_entity_index),
                    "final released holder insertion must restore current map membership"
                );
            }
        }
        self.finalize_supported_renderable_records()
            .expect("validated source renderable active set");
        if let Some(fire) = self.context.spell_fire.as_mut() {
            // The class-15 handlers consume one active tick after their
            // same-turn projectile factories: model 0 in `sub_693F0` and
            // model 1 in `sub_69640`. This lets Fireball and Possession return
            // to their inactive, retriggerable state on the source clock.
            for holder in &mut fire.holders {
                if matches!(holder.model, 0 | 1) && holder.active_value > 0 {
                    holder.active_value -= 1;
                }
            }
        }
        self.mission.sync_collected_spell_holders(&mut self.context);
        self.mission
            .sync_speed_holder_projection(&mut self.state, &mut self.context);
        let state_hash = state_hash(&self.state);
        let typed_events = self.context.events.clone();
        for event in typed_events {
            let record = self.encode_flight_event(event, state_hash);
            self.append_event(record);
        }
        let live_world_hash = self.live_world_hash_v2();
        for (event, random_state) in goat_events {
            let record = self.encode_goat_event(event, live_world_hash, random_state);
            self.append_event(record);
        }
        let mission_events = self.mission.events().to_vec();
        for event in mission_events {
            let record = self.encode_mission_event(event, state_hash);
            self.append_event(record);
        }
        self.drain_pending_audio_commands();
        for pass in evidence {
            let mut record = self.next_event_base(MC_EVENT_MOVEMENT_RESOLUTION, pass.state_hash);
            record.code = resolution_code(&pass);
            self.last_resolution = u8::try_from(record.code).unwrap_or(u8::MAX);
            self.append_event(record);
        }
        // The original loop stops music after its first DrawAndEvents pass,
        // then starts Night logical track 1 after the second pass while its
        // local `gameTurn` is 1. Keep that process-loop event distinct from
        // the per-player UpdateMusic envelope command.
        if self.music_start_armed && self.source_turn == 2 {
            let mut record = self.next_event_base(MC_EVENT_START_MUSIC, state_hash);
            record.code = u32::try_from(self.world.runtime().maptype_music)
                .expect("qualified Level 1 music selector is non-negative");
            record.value_0 = 0;
            record.value_1 = 1;
            record.asset_ref_index = self.world.music_asset_ref().unwrap_or(MC_ASSET_REF_NONE);
            self.append_event(record);
            self.music_start_armed = false;
        }
    }

    pub(crate) fn apply_goat_event_side_effects(
        &mut self,
        _world: &NativeWorld,
        events: &[GoatStepEvent],
    ) {
        for event in events {
            match *event {
                GoatStepEvent::ManaConverted { entity_index, mana } => {
                    if let Some((position, yaw, rng)) = self
                        .live_world
                        .goats()
                        .iter()
                        .find(|goat| goat.entity_index == entity_index)
                        .map(|goat| (goat.position, goat.yaw, goat.rng))
                    {
                        self.mission.spawn_mana_sphere_from_death(
                            &self.terrain,
                            entity_index,
                            position,
                            yaw,
                            rng,
                            mana,
                            0,
                        );
                        self.mission.disable_imported_source_slot(entity_index);
                    }
                }
                GoatStepEvent::ExtraManaSphereRequested { entity_index } => {
                    if let Some(position) = self
                        .live_world
                        .goats()
                        .iter()
                        .find(|goat| goat.entity_index == entity_index)
                        .map(|goat| goat.position)
                    {
                        self.mission
                            .spawn_death_burst_from_death(entity_index, position);
                    }
                }
                GoatStepEvent::Reclaimed { .. } => {}
                _ => {}
            }
        }
    }

    pub(crate) fn apply_house_event_side_effects(
        &mut self,
        events: &[HouseStepEvent],
        frozen_source_views: &[Level1SourceEntityView],
    ) {
        for event in events {
            match *event {
                HouseStepEvent::ConstructionPass {
                    entity_index,
                    source_id,
                    position,
                    building_index,
                    remaining,
                    first_pass,
                } => {
                    let footprint = self
                        .world
                        .house_build_footprint(building_index)
                        .expect("Level 1 action-51 house BUILD footprint");
                    if first_pass {
                        let effect_start = self.mission.house_construction_effects().len();
                        self.mission.apply_house_construction_first_pass(
                            frozen_source_views,
                            entity_index,
                            source_id,
                            position,
                            footprint,
                        );
                        let effects =
                            self.mission.house_construction_effects()[effect_start..].to_vec();
                        self.live_world
                            .apply_house_construction_source_effects(&effects);
                    }
                    self.terrain
                        .apply_house_construction(position, Some(remaining), footprint)
                        .expect("qualified Level 1 action-51 construction pass");
                    self.pending_house_effects.push(*event);
                }
                HouseStepEvent::ConstructionCompleted {
                    entity_index,
                    position,
                    building_index,
                    collision_half_width,
                    collision_half_depth,
                    ..
                } => {
                    let footprint = self
                        .world
                        .house_build_footprint(building_index)
                        .expect("Level 1 action-51 house BUILD footprint");
                    self.terrain
                        .apply_house_construction(position, None, footprint)
                        .expect("qualified Level 1 action-51 construction completion");
                    self.mission
                        .apply_house_construction_completion(
                            frozen_source_views,
                            entity_index,
                            position,
                            collision_half_width,
                            collision_half_depth,
                            |index| {
                                self.world
                                    .house_build_footprint(index)
                                    .map(|value| (value.width, value.height))
                            },
                        )
                        .expect("qualified Level 1 action-51 completion links");
                    self.pending_house_effects.push(*event);
                }
                HouseStepEvent::Possessed {
                    entity_index,
                    owner_entity_index,
                    payload,
                } => self.mission.apply_building_possession(
                    &self.state,
                    entity_index,
                    owner_entity_index,
                    payload,
                ),
                HouseStepEvent::RemoveFootprint {
                    entity_index,
                    attacker_entity_index,
                    position,
                    building_index,
                    occupants,
                    rng,
                    ..
                } => {
                    let produces_residents = self
                        .live_world
                        .houses()
                        .iter()
                        .find(|house| house.entity_index == entity_index)
                        .is_some_and(|house| house.parameter.produces_villagers());
                    let footprint = self
                        .world
                        .house_build_footprint(building_index)
                        .cloned()
                        .expect("Level 1 action-53 house BUILD footprint");
                    let outcome = self
                        .terrain
                        .apply_house_removal(
                            position,
                            rng,
                            occupants,
                            produces_residents,
                            &footprint,
                        )
                        .expect("qualified Level 1 house terrain removal");
                    self.live_world
                        .commit_house_removal_state(
                            entity_index,
                            outcome.final_rng,
                            outcome.final_occupants,
                        )
                        .expect("commit action-53 house state");
                    for resident in outcome.residents {
                        self.mission
                            .spawn_house_resident(
                                resident.position,
                                resident.model,
                                resident.action,
                                attacker_entity_index,
                            )
                            .expect("qualified action-53 resident constructor");
                        self.pending_house_effects
                            .push(HouseStepEvent::ResidentRequested {
                                entity_index,
                                attacker_entity_index,
                                position: resident.position,
                                model: resident.model,
                                action: resident.action,
                            });
                    }
                    self.mission.remove_imported_building(entity_index);
                    self.pending_house_effects.push(*event);
                }
                HouseStepEvent::ResidentRequested {
                    attacker_entity_index,
                    position,
                    model,
                    action,
                    ..
                } => {
                    self.mission
                        .spawn_house_resident(position, model, action, attacker_entity_index)
                        .expect("qualified action-52 resident constructor");
                    self.pending_house_effects.push(*event);
                }
                HouseStepEvent::Damaged { .. } | HouseStepEvent::DestructionStarted { .. } => {
                    self.pending_house_effects.push(*event);
                }
            }
        }
    }

    pub(crate) fn reclaim_imported_source_slots(&mut self) -> Vec<GoatStepEvent> {
        self.mission
            .take_reclaimed_imported_source_slots()
            .into_iter()
            .filter_map(|source_slot| self.live_world.reclaim_source_slot(source_slot))
            .collect()
    }

    pub(crate) fn sync_supported_renderable_records(&mut self) -> Result<(), String> {
        for actor in &mut self.frozen_actors {
            if let Some(villager) = self
                .live_world
                .villagers()
                .iter()
                .find(|villager| villager.entity_index == actor.source_entity_index)
            {
                actor.position = villager.position;
                actor.yaw = villager.yaw;
            } else if (130..=132).contains(&actor.source_entity_index) {
                actor.position.z = self.terrain.ground_height(actor.position);
            }
        }
        for goat in self.live_world.goats() {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: goat.entity_index,
                active: goat.active,
                x: goat.position.x,
                y: goat.position.y,
            })?;
        }
        for villager in self.live_world.villagers() {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: villager.entity_index,
                active: villager.active,
                x: villager.position.x,
                y: villager.position.y,
            })?;
        }
        for actor in self.mission.actors().iter().filter(|actor| {
            native_composited_actor_kind(actor.kind) && actor.source_entity_index != 0
        }) {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: actor.source_entity_index,
                active: actor.active,
                x: actor.position.x,
                y: actor.position.y,
            })?;
        }
        for resident in self.mission.dynamic_residents() {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: resident.source_entity_index,
                active: resident.active,
                x: resident.position.x,
                y: resident.position.y,
            })?;
        }
        for segment in self.mission.multipart_segments() {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: segment.source_entity_index,
                active: segment.active,
                x: segment.position.x,
                y: segment.position.y,
            })?;
        }
        for ambient in self.mission.ambient_entities() {
            if ambient.map_linked {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: ambient.source_entity_index,
                    active: ambient.visible(),
                    x: ambient.position.x,
                    y: ambient.position.y,
                })?;
            }
        }
        for projectile in self
            .mission
            .projectiles()
            .iter()
            .filter(|projectile| projectile.source_entity_index() != 0)
        {
            let position = match projectile {
                Level1Projectile::PlayerFireball(value) => value.position,
                Level1Projectile::ArcherArrow(value) => value.position,
                Level1Projectile::FireflyFireball(value) => value.position,
                Level1Projectile::CastleBuild(value) => value.position,
                Level1Projectile::Possession(value) => value.position,
            };
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: projectile.source_entity_index(),
                active: projectile.active(),
                x: position.x,
                y: position.y,
            })?;
        }
        for impact in self
            .mission
            .impacts()
            .iter()
            .filter(|impact| impact.model != POSSESSION_ATTRACTION_MODEL)
        {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: impact.source_entity_index,
                active: impact.active,
                x: impact.position.x,
                y: impact.position.y,
            })?;
        }
        for burst in self
            .mission
            .death_bursts()
            .iter()
            .filter(|burst| burst.active)
        {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: burst.source_entity_index,
                active: true,
                x: burst.position.x,
                y: burst.position.y,
            })?;
        }
        if let Some(marker) = self
            .mission
            .player_death_marker()
            .filter(|marker| marker.active)
        {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: marker.source_entity_index,
                active: true,
                x: marker.position.x,
                y: marker.position.y,
            })?;
        }
        for building in self.mission.possessed_buildings() {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: building.entity_index,
                active: building.active,
                x: building.position.x,
                y: building.position.y,
            })?;
        }
        if self.mission.castle_exists && self.mission.castle_source_entity_index != 0 {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: self.mission.castle_source_entity_index,
                active: true,
                x: self.mission.castle_position.x,
                y: self.mission.castle_position.y,
            })?;
        }
        for sphere in self
            .mission
            .mana_spheres()
            .iter()
            .filter(|sphere| sphere.source_entity_index != 0)
        {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: sphere.source_entity_index,
                active: sphere.active && !sphere.pending_delete,
                x: sphere.position.x,
                y: sphere.position.y,
            })?;
        }
        if let Some(carrier) = self
            .mission
            .mana_carrier()
            .filter(|carrier| carrier.source_entity_index != 0)
        {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: carrier.source_entity_index,
                active: carrier.active,
                x: carrier.position.x,
                y: carrier.position.y,
            })?;
        }
        Ok(())
    }

    pub(crate) fn sync_source_slot_renderable_records(
        &mut self,
        source_slot: u16,
    ) -> Result<(), String> {
        if self.renderable_map_chains.tracks(source_slot) {
            if source_slot == self.state.entity_index {
                let seed = RenderableMapSeed {
                    entity_index: source_slot,
                    active: true,
                    x: self.state.position.x,
                    y: self.state.position.y,
                };
                self.mission.sync_source_map_entity(seed)?;
                self.renderable_map_chains.sync_entity(seed)?;
            } else if let Some(goat) = self
                .live_world
                .goats()
                .iter()
                .find(|goat| goat.entity_index == source_slot)
            {
                let seed = RenderableMapSeed {
                    entity_index: goat.entity_index,
                    active: goat.active,
                    x: goat.position.x,
                    y: goat.position.y,
                };
                self.mission.sync_source_map_entity(seed)?;
                self.renderable_map_chains.sync_entity(seed)?;
            } else if let Some(villager) = self
                .live_world
                .villagers()
                .iter()
                .find(|villager| villager.entity_index == source_slot)
            {
                if let Some(actor) = self
                    .frozen_actors
                    .iter_mut()
                    .find(|actor| actor.source_entity_index == source_slot)
                {
                    actor.position = villager.position;
                    actor.yaw = villager.yaw;
                }
                let seed = RenderableMapSeed {
                    entity_index: villager.entity_index,
                    active: villager.active,
                    x: villager.position.x,
                    y: villager.position.y,
                };
                self.mission.sync_source_map_entity(seed)?;
                self.renderable_map_chains.sync_entity(seed)?;
            } else if let Some(actor) = self.mission.actors().iter().find(|actor| {
                native_composited_actor_kind(actor.kind) && actor.source_entity_index == source_slot
            }) {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: actor.source_entity_index,
                    active: actor.active,
                    x: actor.position.x,
                    y: actor.position.y,
                })?;
            } else if let Some(resident) = self
                .mission
                .dynamic_residents()
                .find(|resident| resident.source_entity_index == source_slot)
            {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: resident.source_entity_index,
                    active: resident.active,
                    x: resident.position.x,
                    y: resident.position.y,
                })?;
            } else if let Some(segment) = self
                .mission
                .multipart_segments()
                .iter()
                .find(|segment| segment.source_entity_index == source_slot)
            {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: segment.source_entity_index,
                    active: segment.active,
                    x: segment.position.x,
                    y: segment.position.y,
                })?;
            } else if let Some(ambient) = self
                .mission
                .ambient_entities()
                .find(|ambient| ambient.source_entity_index == source_slot)
            {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: ambient.source_entity_index,
                    active: ambient.visible(),
                    x: ambient.position.x,
                    y: ambient.position.y,
                })?;
            } else if let Some(building) = self
                .mission
                .possessed_buildings()
                .iter()
                .find(|building| building.entity_index == source_slot)
            {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: building.entity_index,
                    active: building.active,
                    x: building.position.x,
                    y: building.position.y,
                })?;
            } else if self.mission.castle_exists
                && self.mission.castle_source_entity_index == source_slot
            {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: true,
                    x: self.mission.castle_position.x,
                    y: self.mission.castle_position.y,
                })?;
            } else if let Some(sphere) = self
                .mission
                .mana_spheres()
                .iter()
                .find(|sphere| sphere.source_entity_index == source_slot)
            {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: sphere.active && !sphere.pending_delete,
                    x: sphere.position.x,
                    y: sphere.position.y,
                })?;
            } else if let Some(carrier) = self
                .mission
                .mana_carrier()
                .filter(|carrier| carrier.source_entity_index == source_slot)
            {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: carrier.active,
                    x: carrier.position.x,
                    y: carrier.position.y,
                })?;
            }
        }
        // Replay each constructor's AddEventToMap order. Ordinary actors map
        // immediately after allocation. The multipart constructor is the
        // exception: it maps segment 1..16 first and its already-allocated
        // head last, yielding head -> segment16 -> ... -> segment1.
        let mut new_actors = self
            .mission
            .actors()
            .iter()
            .filter(|actor| {
                native_composited_actor_kind(actor.kind)
                    && actor.source_entity_index != 0
                    && !self.renderable_map_chains.tracks(actor.source_entity_index)
            })
            .collect::<Vec<_>>();
        new_actors.sort_unstable_by_key(|actor| actor.source_entity_index);
        let mut newly_tracked = Vec::new();
        for actor in new_actors {
            if actor.kind == Level1ActorKind::MultipartHead {
                let mut segments = self
                    .mission
                    .multipart_segments()
                    .iter()
                    .filter(|segment| {
                        segment.head_source_entity_index == actor.source_entity_index
                            && !self
                                .renderable_map_chains
                                .tracks(segment.source_entity_index)
                    })
                    .collect::<Vec<_>>();
                segments.sort_unstable_by_key(|segment| segment.ordinal);
                newly_tracked.extend(segments.into_iter().map(|segment| RenderableMapSeed {
                    entity_index: segment.source_entity_index,
                    active: segment.active,
                    x: segment.position.x,
                    y: segment.position.y,
                }));
            }
            newly_tracked.push(RenderableMapSeed {
                entity_index: actor.source_entity_index,
                active: actor.active,
                x: actor.position.x,
                y: actor.position.y,
            });
        }
        // Preserve partial-construction prefixes if the source arena filled
        // before all children could be allocated.
        let orphan_segments = self
            .mission
            .multipart_segments()
            .iter()
            .filter(|segment| {
                !self
                    .renderable_map_chains
                    .tracks(segment.source_entity_index)
                    && !newly_tracked
                        .iter()
                        .any(|seed| seed.entity_index == segment.source_entity_index)
            })
            .map(|segment| RenderableMapSeed {
                entity_index: segment.source_entity_index,
                active: segment.active,
                x: segment.position.x,
                y: segment.position.y,
            })
            .collect::<Vec<_>>();
        newly_tracked.extend(orphan_segments);
        for seed in newly_tracked {
            self.renderable_map_chains.sync_entity(seed)?;
        }
        let newly_tracked_residents = self
            .mission
            .dynamic_residents()
            .filter(|resident| {
                resident.source_entity_index != 0
                    && !self
                        .renderable_map_chains
                        .tracks(resident.source_entity_index)
            })
            .map(|resident| RenderableMapSeed {
                entity_index: resident.source_entity_index,
                active: resident.active,
                x: resident.position.x,
                y: resident.position.y,
            })
            .collect::<Vec<_>>();
        for seed in newly_tracked_residents {
            self.renderable_map_chains.sync_entity(seed)?;
        }
        let newly_tracked_ambient = self
            .mission
            .ambient_entities()
            .filter(|ambient| {
                ambient.map_linked
                    && !self
                        .renderable_map_chains
                        .tracks(ambient.source_entity_index)
            })
            .map(|ambient| RenderableMapSeed {
                entity_index: ambient.source_entity_index,
                active: ambient.visible(),
                x: ambient.position.x,
                y: ambient.position.y,
            })
            .collect::<Vec<_>>();
        for seed in newly_tracked_ambient {
            self.renderable_map_chains.sync_entity(seed)?;
        }
        if self.mission.castle_exists
            && self.mission.castle_source_entity_index != 0
            && !self
                .renderable_map_chains
                .tracks(self.mission.castle_source_entity_index)
        {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: self.mission.castle_source_entity_index,
                active: true,
                x: self.mission.castle_position.x,
                y: self.mission.castle_position.y,
            })?;
        }
        let newly_tracked_spheres = self
            .mission
            .mana_spheres()
            .iter()
            .filter(|sphere| {
                sphere.source_entity_index != 0
                    && !self
                        .renderable_map_chains
                        .tracks(sphere.source_entity_index)
            })
            .map(|sphere| RenderableMapSeed {
                entity_index: sphere.source_entity_index,
                active: sphere.active,
                x: sphere.position.x,
                y: sphere.position.y,
            })
            .collect::<Vec<_>>();
        for seed in newly_tracked_spheres {
            self.renderable_map_chains.sync_entity(seed)?;
        }
        if let Some(carrier) = self.mission.mana_carrier().filter(|carrier| {
            carrier.source_entity_index != 0
                && !self
                    .renderable_map_chains
                    .tracks(carrier.source_entity_index)
        }) {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: carrier.source_entity_index,
                active: carrier.active,
                x: carrier.position.x,
                y: carrier.position.y,
            })?;
        }
        Ok(())
    }

    pub(crate) fn finalize_supported_renderable_records(&mut self) -> Result<(), String> {
        let mut active = self
            .world
            .entities()
            .iter()
            .filter(|entity| {
                entity.active
                    && entity.control_flags & 8 != 0
                    && self.mission.imported_source_slot_is_live(entity.index)
            })
            .map(|entity| entity.index)
            .collect::<Vec<_>>();
        active.extend(
            self.live_world
                .goats()
                .iter()
                .filter(|goat| goat.active)
                .map(|goat| goat.entity_index),
        );
        active.extend(
            self.live_world
                .villagers()
                .iter()
                .filter(|villager| villager.active)
                .map(|villager| villager.entity_index),
        );
        active.extend(
            self.frozen_actors
                .iter()
                .filter(|actor| {
                    actor.source_entity_index >= 130
                        && self
                            .mission
                            .imported_source_slot_is_live(actor.source_entity_index)
                })
                .map(|actor| actor.source_entity_index),
        );
        active.extend(
            self.mission
                .actors()
                .iter()
                .filter(|actor| {
                    native_composited_actor_kind(actor.kind)
                        && actor.source_entity_index != 0
                        && actor.active
                })
                .map(|actor| actor.source_entity_index),
        );
        active.extend(
            self.mission
                .dynamic_residents()
                .filter(|resident| resident.active)
                .map(|resident| resident.source_entity_index),
        );
        active.extend(
            self.mission
                .multipart_segments()
                .iter()
                .filter(|segment| segment.active)
                .map(|segment| segment.source_entity_index),
        );
        active.extend(
            self.mission
                .ambient_entities()
                .filter(|ambient| ambient.visible())
                .map(|ambient| ambient.source_entity_index),
        );
        active.extend(
            self.mission
                .projectiles()
                .iter()
                .filter(|projectile| projectile.active())
                .map(|projectile| projectile.source_entity_index()),
        );
        active.extend(
            self.mission
                .impacts()
                .iter()
                .filter(|impact| impact.active && impact.model != POSSESSION_ATTRACTION_MODEL)
                .map(|impact| impact.source_entity_index),
        );
        active.extend(
            self.mission
                .death_bursts()
                .iter()
                .filter(|burst| burst.active)
                .map(|burst| burst.source_entity_index),
        );
        active.extend(
            self.mission
                .released_spell_holders()
                .iter()
                .flatten()
                .map(|holder| holder.source_entity_index),
        );
        if let Some(marker) = self
            .mission
            .player_death_marker()
            .filter(|marker| marker.active)
        {
            active.push(marker.source_entity_index);
        }
        active.extend(
            self.mission
                .possessed_buildings()
                .iter()
                .filter(|building| building.active)
                .map(|building| building.entity_index),
        );
        if self.mission.castle_exists && self.mission.castle_source_entity_index != 0 {
            active.push(self.mission.castle_source_entity_index);
        }
        active.extend(
            self.mission
                .mana_spheres()
                .iter()
                .filter(|sphere| {
                    sphere.active && !sphere.pending_delete && sphere.source_entity_index != 0
                })
                .map(|sphere| sphere.source_entity_index),
        );
        if let Some(carrier) = self
            .mission
            .mana_carrier()
            .filter(|carrier| carrier.active && carrier.source_entity_index != 0)
        {
            active.push(carrier.source_entity_index);
        }
        // Hidden class-10/model-45 houses remain live collision rows even
        // before Possession reveals their coloured marker. Keeping them in
        // the filtered chain preserves their source painter/collision order.
        active.extend(
            self.mission
                .possession_collision_targets()
                .iter()
                .filter(|target| target.active)
                .map(|target| target.entity_index),
        );
        active.sort_unstable();
        active.dedup();
        self.renderable_map_chains.retain_entities(&active)
    }

    pub(crate) fn inject_new_fireball_impacts(&mut self, event_start: usize) {
        let impacts = self.mission.events()[event_start..]
            .iter()
            .filter_map(|event| {
                let Level1MissionEvent::FireballImpacted {
                    impact_id,
                    position,
                    ..
                } = event
                else {
                    return None;
                };
                let payload = self
                    .mission
                    .impacts()
                    .iter()
                    .find(|impact| impact.id == *impact_id)
                    .map(|impact| {
                        (
                            impact.payload,
                            impact.owner_entity_index,
                            impact.external_target_entity_index,
                        )
                    })?;
                Some((*position, payload.0, payload.1, payload.2))
            })
            .collect::<Vec<_>>();
        for (position, payload, owner_entity_index, direct_target_entity_index) in impacts {
            let targets = self
                .live_world
                .goats()
                .iter()
                .filter(|goat| {
                    goat.active
                        && goat.entity_index != owner_entity_index
                        && (goat.entity_index == direct_target_entity_index
                            || sim_core::level1_mission::source_entities_overlap(
                                position,
                                sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                                sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_VERTICAL_OFFSET,
                                sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_HEIGHT,
                                goat.position,
                                goat.collision_half_width,
                                goat.collision_vertical_offset,
                                goat.collision_half_height,
                            ))
                })
                .map(|goat| goat.entity_index)
                .chain(
                    self.live_world
                        .villagers()
                        .iter()
                        .filter(|villager| {
                            villager.active
                                && villager.entity_index != owner_entity_index
                                && (villager.entity_index == direct_target_entity_index
                                    || sim_core::level1_mission::source_entities_overlap(
                                        position,
                                        sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                                        sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_VERTICAL_OFFSET,
                                        sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_HEIGHT,
                                        villager.position,
                                        villager.collision_half_width,
                                        villager.collision_vertical_offset,
                                        villager.collision_half_height,
                                    ))
                        })
                        .map(|villager| villager.entity_index),
                )
                .chain(
                    self.live_world
                        .houses()
                        .iter()
                        .filter(|house| {
                            house.active
                                && house.entity_index != owner_entity_index
                                && (house.entity_index == direct_target_entity_index
                                    || sim_core::level1_mission::source_entities_overlap_xy(
                                        position,
                                        sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                                        sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                                        house.position,
                                        house.collision_half_width,
                                        house.collision_half_depth,
                                    ))
                        })
                        .map(|house| house.entity_index),
                )
                .collect::<Vec<_>>();
            for target in targets {
                let _ = self.live_world.inject_damage(
                    target,
                    payload,
                    owner_entity_index.cast_signed(),
                );
            }
        }
    }

    pub(crate) fn drain_pending_audio_commands(&mut self) {
        let unresolved = self
            .audio_commands
            .iter()
            .filter_map(|(sequence, command)| {
                (command.action == 0).then_some((*sequence, *command))
            })
            .collect::<Vec<_>>();
        let pooled_flush_id = unresolved
            .iter()
            .filter(|(_, command)| !matches!(command.sound_id, 1 | 2 | 5 | 31))
            .map(|(sequence, _)| *sequence)
            .min()
            .unwrap_or(0);
        let mut pending = BTreeMap::<u16, u64>::new();
        for (sequence, command) in unresolved {
            if matches!(command.sound_id, 1 | 2 | 5 | 31) {
                let mut command = command;
                let (action, voice_index) = self.allocate_audio_voice(command);
                command.action = if action == MC_AUDIO_ACTION_SUPPRESS_EXISTING {
                    MC_AUDIO_ACTION_UPDATE_EXISTING
                } else {
                    action
                };
                command.voice_index = voice_index;
                command.pooled_flush_rank = 0;
                self.audio_commands.insert(sequence, command);
                continue;
            }
            if let Some(current_sequence) = pending.get(&command.sound_id).copied() {
                let current = self.audio_commands[&current_sequence];
                if positional_sound_should_replace(
                    i32::from(command.volume_q15),
                    i32::from(current.volume_q15),
                ) {
                    self.audio_commands
                        .get_mut(&current_sequence)
                        .expect("pending command remains present")
                        .action = MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED;
                    pending.insert(command.sound_id, sequence);
                } else {
                    self.audio_commands
                        .get_mut(&sequence)
                        .expect("new command remains present")
                        .action = MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED;
                }
            } else {
                pending.insert(command.sound_id, sequence);
            }
        }
        let mut pooled_flush_rank = 1_u8;
        for sequence in pending.into_values() {
            let mut command = self.audio_commands[&sequence];
            let (action, voice_index) = self.allocate_audio_voice(command);
            command.action = action;
            command.voice_index = voice_index;
            if matches!(
                command.action,
                MC_AUDIO_ACTION_START | MC_AUDIO_ACTION_RESTART | MC_AUDIO_ACTION_UPDATE_EXISTING
            ) {
                command.pooled_flush_rank = pooled_flush_rank;
                command.reserved[0] = pooled_flush_id as u32;
                command.reserved[1] = (pooled_flush_id >> 32) as u32;
                pooled_flush_rank = pooled_flush_rank
                    .checked_add(1)
                    .expect("ordinary sound pool has at most 70 winners");
            } else {
                command.pooled_flush_rank = 0;
            }
            self.audio_commands.insert(sequence, command);
        }
    }

    pub(crate) fn insert_prepared_audio_command(
        &mut self,
        sequence: u64,
        mut command: McAudioCommandV2,
    ) {
        if self.game_paused
            && command.action == 0
            && !matches!(command.sound_id, 1 | 2 | 5 | 31)
            && let Some(current_sequence) =
                self.audio_commands
                    .iter()
                    .find_map(|(candidate_sequence, candidate)| {
                        (candidate.action == 0 && candidate.sound_id == command.sound_id)
                            .then_some(*candidate_sequence)
                    })
        {
            let current = self.audio_commands[&current_sequence];
            if positional_sound_should_replace(
                i32::from(command.volume_q15),
                i32::from(current.volume_q15),
            ) {
                self.audio_commands
                    .get_mut(&current_sequence)
                    .expect("paused pending command remains present")
                    .action = MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED;
            } else {
                command.action = MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED;
            }
        }
        self.audio_commands.insert(sequence, command);
    }

    pub(crate) fn allocate_audio_voice(&mut self, command: McAudioCommandV2) -> (u8, u16) {
        let matching = self.audio_voices.iter().position(|voice| {
            voice.is_some_and(|voice| {
                voice.sound_id == command.sound_id && voice.sound_flags == command.sound_flags
            })
        });
        // `play_type` is the queued `EntitySounds.playType_0` value for
        // PrepareEventSound events (1 = restart-or-start, 3 = if-not-playing).
        // The four direct ambience loops retain the lower PlaySample enum
        // value 2 for if-not-playing.
        let is_direct_ambience_if_not_playing = matches!(command.sound_id, 1 | 2 | 5 | 31)
            && command.play_type == POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING;
        if (command.play_type == 3 || is_direct_ambience_if_not_playing)
            && let Some(index) = matching
        {
            return (
                MC_AUDIO_ACTION_SUPPRESS_EXISTING,
                u16::try_from(index).expect("ten voices fit u16"),
            );
        }
        if command.play_type == 1
            && let Some(index) = matching
        {
            self.audio_voices[index] = Some(AudioVoice {
                sound_id: command.sound_id,
                sound_flags: command.sound_flags,
                event_sequence: command.event_sequence,
            });
            return (
                MC_AUDIO_ACTION_RESTART,
                u16::try_from(index).expect("ten voices fit u16"),
            );
        }
        let Some(index) = self.audio_voices.iter().position(Option::is_none) else {
            return (MC_AUDIO_ACTION_SUPPRESS_NO_VOICE, MC_AUDIO_VOICE_NONE);
        };
        self.audio_voices[index] = Some(AudioVoice {
            sound_id: command.sound_id,
            sound_flags: command.sound_flags,
            event_sequence: command.event_sequence,
        });
        (
            MC_AUDIO_ACTION_START,
            u16::try_from(index).expect("ten voices fit u16"),
        )
    }

    pub(crate) fn encode_flight_event(&mut self, event: FlightEvent, hash: u64) -> McEventV2 {
        match event {
            FlightEvent::PrepareSound {
                entity_index,
                player_index,
                sound_id,
            } => {
                let mut record = self.next_event_base(MC_EVENT_PREPARE_SOUND, hash);
                record.entity_index = entity_index;
                record.player_index = player_index;
                record.code = u32::from(sound_id);
                record.asset_ref_index = self
                    .world
                    .sound_asset_ref(sound_id)
                    .unwrap_or(MC_ASSET_REF_NONE);
                if let Some(command) = self.prepare_audio_command(&record) {
                    self.insert_prepared_audio_command(record.sequence, command);
                }
                record
            }
            FlightEvent::EndLoop {
                entity_index,
                player_index,
                sound_id,
            } => {
                let mut record = self.next_event_base(MC_EVENT_END_LOOP, hash);
                record.entity_index = entity_index;
                record.player_index = player_index;
                record.code = u32::from(sound_id);
                record.asset_ref_index = self
                    .world
                    .sound_asset_ref(sound_id)
                    .unwrap_or(MC_ASSET_REF_NONE);
                if matches!(sound_id, 1 | 2 | 5 | 31) {
                    let matching = self.audio_voices.iter().position(|voice| {
                        voice.is_some_and(|voice| {
                            voice.sound_id == sound_id && voice.sound_flags == 0
                        })
                    });
                    let mut command = local_audio_command_record(
                        &record,
                        sound_id,
                        0,
                        POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING,
                    );
                    if let Some(index) = matching {
                        command.action = MC_AUDIO_ACTION_END_LOOP;
                        command.voice_index = u16::try_from(index).expect("ten voices fit u16");
                    } else {
                        command.action = MC_AUDIO_ACTION_SUPPRESS_NO_EXISTING;
                    }
                    self.audio_commands.insert(record.sequence, command);
                }
                record
            }
            FlightEvent::UpdateMusic {
                mode,
                timer_multiplier,
            } => {
                let mut record = self.next_event_base(MC_EVENT_UPDATE_MUSIC, hash);
                record.code = u32::from(mode);
                record.value_0 = i32::from(timer_multiplier);
                record.value_1 = self.source_turn;
                record.asset_ref_index = self.world.music_asset_ref().unwrap_or(MC_ASSET_REF_NONE);
                record
            }
            FlightEvent::RequestSpellActivation {
                source,
                holder_entity_index,
                direction_mask,
            } => {
                self.fire_sequence = self.fire_sequence.wrapping_add(1);
                let mut record = self.next_event_base(MC_EVENT_REQUEST_SPELL_ACTIVATION, hash);
                record.entity_index = holder_entity_index;
                record.code = match source {
                    sim_core::SpellFireSource::Left => 0,
                    sim_core::SpellFireSource::Right => 1,
                    sim_core::SpellFireSource::Selected => 2,
                };
                record.value_0 = i32::from(direction_mask);
                record
            }
            FlightEvent::ShowHelpHint { hint_index, state } => {
                let mut record = self.next_event_base(MC_EVENT_SHOW_HELP_HINT, hash);
                record.code = u32::from(hint_index);
                record.value_0 = i32::from(state);
                record
            }
        }
    }

    pub(crate) fn encode_goat_event(
        &mut self,
        event: GoatStepEvent,
        live_hash: u64,
        random_state: u32,
    ) -> McEventV2 {
        match event {
            GoatStepEvent::Bleat {
                entity_index,
                sound_id,
                position,
                action,
            } => {
                let mut record = self.next_event_base(MC_EVENT_GOAT_SOUND_REQUEST, live_hash);
                record.entity_index = entity_index;
                record.code = u32::from(sound_id);
                record.asset_ref_index = self
                    .world
                    .sound_asset_ref(sound_id)
                    .unwrap_or(MC_ASSET_REF_NONE);
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    entity_index,
                    position,
                    action,
                    random_state,
                );
                record
            }
            GoatStepEvent::ManaConverted { entity_index, mana } => {
                let mut record = self.next_event_base(MC_EVENT_GOAT_MANA_CONVERTED, live_hash);
                record.entity_index = entity_index;
                record.value_0 = mana;
                record
            }
            GoatStepEvent::ExtraManaSphereRequested { entity_index } => {
                let mut record =
                    self.next_event_base(MC_EVENT_GOAT_EXTRA_MANA_SPHERE_REQUEST, live_hash);
                record.entity_index = entity_index;
                record
            }
            GoatStepEvent::RemovalQueued { entity_index } => {
                let mut record = self.next_event_base(MC_EVENT_GOAT_REMOVAL_QUEUED, live_hash);
                record.entity_index = entity_index;
                record
            }
            GoatStepEvent::Reclaimed { entity_index } => {
                let mut record = self.next_event_base(MC_EVENT_GOAT_RECLAIMED, live_hash);
                record.entity_index = entity_index;
                record
            }
            GoatStepEvent::EngagementBoundaryReached {
                entity_index,
                target_entity_index,
            } => {
                let mut record = self.next_event_base(MC_EVENT_GOAT_ENGAGEMENT_BOUNDARY, live_hash);
                record.entity_index = entity_index;
                record.value_0 = i32::from(target_entity_index);
                record
            }
        }
    }

    pub(crate) fn encode_mission_event(
        &mut self,
        event: Level1MissionEvent,
        hash: u64,
    ) -> McEventV2 {
        match event {
            Level1MissionEvent::ObjectiveAdvanced {
                completed_stage,
                text_selector,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_OBJECTIVE_ADVANCED, hash);
                record.code = u32::from(text_selector);
                record.value_0 = i32::from(completed_stage);
                record
            }
            Level1MissionEvent::AbilityImproved {
                source_family,
                notification_template_selector,
                ability_name_selector,
                notification_state,
                notification_timer,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_ABILITY_IMPROVED, hash);
                record.code = u32::from(source_family);
                record.entity_index = ability_name_selector;
                record.value_0 = i32::from(notification_state);
                record.value_1 = i32::from(notification_timer);
                record.reserved[0] = u32::from(notification_template_selector);
                record
            }
            Level1MissionEvent::SpellCollected {
                family,
                authored_index,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_SPELL_COLLECTED, hash);
                record.code = u32::from(family);
                record.entity_index = authored_index;
                record
            }
            Level1MissionEvent::FireballSpawned { projectile_id } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_FIREBALL_SPAWNED, hash);
                record.code = projectile_id;
                record
            }
            Level1MissionEvent::PossessionSpawned { projectile_id } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_POSSESSION_SPAWNED, hash);
                record.code = projectile_id;
                record
            }
            Level1MissionEvent::ManaSphereSpawned {
                sphere_id,
                source_entity_index,
                mana,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_MANA_SPHERE_SPAWNED, hash);
                record.code = sphere_id;
                record.entity_index = source_entity_index;
                record.value_0 = mana;
                record
            }
            Level1MissionEvent::ManaSphereClaimed {
                sphere_id,
                owner_entity_index,
                sound_id,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_MANA_SPHERE_CLAIMED, hash);
                record.code = u32::from(sound_id);
                record.entity_index = owner_entity_index;
                record.value_0 = i32::try_from(sphere_id).unwrap_or(i32::MAX);
                let sphere = self
                    .mission
                    .mana_spheres()
                    .iter()
                    .find(|sphere| sphere.id == sphere_id)
                    .copied();
                let position = if owner_entity_index == self.state.entity_index {
                    self.state.position
                } else {
                    self.mission
                        .actors()
                        .iter()
                        .find(|actor| actor.source_entity_index == owner_entity_index)
                        .map(|actor| actor.position)
                        .or_else(|| {
                            self.live_world
                                .goats()
                                .iter()
                                .find(|goat| goat.entity_index == owner_entity_index)
                                .map(|goat| goat.position)
                        })
                        .or_else(|| sphere.map(|sphere| sphere.position))
                        .unwrap_or(self.state.position)
                };
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    owner_entity_index,
                    position,
                    10,
                    self.mission.global_rng(),
                );
                record
            }
            Level1MissionEvent::ManaSphereCollected {
                sphere_id,
                carrier_id,
                mana,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_MANA_SPHERE_COLLECTED, hash);
                record.code = carrier_id;
                record.value_0 = i32::try_from(sphere_id).unwrap_or(i32::MAX);
                record.value_1 = mana;
                record
            }
            Level1MissionEvent::ArcherArrowSpawned {
                projectile_id,
                archer_authored_index,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_ARCHER_ARROW_SPAWNED, hash);
                record.code = projectile_id;
                record.entity_index = archer_authored_index;
                record
            }
            Level1MissionEvent::ArcherArrowSound {
                projectile_id,
                sound_id,
                source_entity_index,
                position,
                random_state,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_ARCHER_ARROW_SOUND, hash);
                record.code = u32::from(sound_id);
                record.value_0 = i32::try_from(projectile_id).unwrap_or(i32::MAX);
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    source_entity_index,
                    position,
                    13,
                    random_state,
                );
                record
            }
            Level1MissionEvent::FireflyFireballSpawned {
                projectile_id,
                firefly_authored_index,
            } => {
                let mut record =
                    self.next_event_base(MC_EVENT_LEVEL1_FIREFLY_FIREBALL_SPAWNED, hash);
                record.code = projectile_id;
                record.entity_index = firefly_authored_index;
                record
            }
            Level1MissionEvent::FireflyChargeSound {
                firefly_authored_index,
                sound_id,
                random_state,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_FIREFLY_CHARGE_SOUND, hash);
                record.code = u32::from(sound_id);
                record.entity_index = firefly_authored_index;
                if let Some(actor) = self
                    .mission
                    .actors()
                    .iter()
                    .find(|actor| actor.authored_index == firefly_authored_index)
                    .copied()
                {
                    self.attach_mission_sound(
                        &mut record,
                        sound_id,
                        actor.source_entity_index,
                        actor.position,
                        actor.action,
                        random_state,
                    );
                }
                record
            }
            Level1MissionEvent::FireballImpacted {
                projectile_id,
                impact_id,
                position,
                sound_id,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_FIREBALL_IMPACTED, hash);
                record.code = impact_id;
                record.value_0 = i32::try_from(projectile_id).unwrap_or(i32::MAX);
                record.value_1 = i32::from(position.z);
                let source_entity_index = self
                    .mission
                    .impacts()
                    .iter()
                    .find(|impact| impact.id == impact_id)
                    .map_or(0, |impact| impact.source_entity_index);
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    source_entity_index,
                    position,
                    0,
                    self.mission.global_rng(),
                );
                record
            }
            Level1MissionEvent::WaterSplashed {
                projectile_id,
                splash_id,
                position,
                sound_id,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_WATER_SPLASHED, hash);
                record.code = u32::from(sound_id);
                record.value_0 = i32::try_from(projectile_id).unwrap_or(i32::MAX);
                record.value_1 = i32::from(position.z);
                let source_entity_index = self
                    .mission
                    .impacts()
                    .iter()
                    .find(|impact| impact.id == splash_id)
                    .map_or(0, |impact| impact.source_entity_index);
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    source_entity_index,
                    position,
                    5,
                    self.mission.global_rng(),
                );
                // The stable event row exposes the splash impact identity to
                // presentation.  Audio correlation uses the impact's source
                // slot internally, but must not replace that presentation key
                // in the shared C ABI row.
                record.entity_index = u16::try_from(splash_id).unwrap_or(u16::MAX);
                record
            }
            Level1MissionEvent::ActorDamaged {
                authored_index,
                attacker_entity_index,
                damage,
                life,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_ACTOR_DAMAGED, hash);
                record.entity_index = authored_index;
                record.code = u32::from(attacker_entity_index);
                record.value_0 = damage;
                record.value_1 = life;
                record
            }
            Level1MissionEvent::ActorKilled {
                authored_index,
                kind,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_ACTOR_KILLED, hash);
                record.entity_index = authored_index;
                record.code = u32::from(kind as u8);
                record
            }
            Level1MissionEvent::CastleBuilt { level } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_CASTLE_BUILT, hash);
                record.code = u32::from(level);
                record
            }
            Level1MissionEvent::CastleProjectileSpawned {
                projectile_id,
                sound_id,
                source_entity_index,
                position,
                random_state,
            } => {
                let mut record =
                    self.next_event_base(MC_EVENT_LEVEL1_CASTLE_PROJECTILE_SPAWNED, hash);
                record.code = u32::from(sound_id);
                record.value_0 = i32::try_from(projectile_id).unwrap_or(i32::MAX);
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    source_entity_index,
                    position,
                    20,
                    random_state,
                );
                record
            }
            Level1MissionEvent::CastlePlacementRejected { projectile_id } => {
                let mut record =
                    self.next_event_base(MC_EVENT_LEVEL1_CASTLE_PLACEMENT_REJECTED, hash);
                record.code = projectile_id;
                record
            }
            Level1MissionEvent::CastleConstructionSound {
                sound_id,
                source_entity_index,
                position,
            } => {
                let mut record =
                    self.next_event_base(MC_EVENT_LEVEL1_CASTLE_CONSTRUCTION_SOUND, hash);
                record.code = u32::from(sound_id);
                record.entity_index = source_entity_index;
                record.value_0 = i32::from(position.x) | (i32::from(position.y) << 16);
                record.value_1 = i32::from(position.z);
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    source_entity_index,
                    position,
                    44,
                    self.mission.global_rng(),
                );
                record
            }
            Level1MissionEvent::CastleDefenseShot {
                controller_entity_index,
                target_entity_index,
                projectile_entity_index,
                spell_family,
                subspell,
                yaw,
                pitch,
                first_shot,
            } => {
                let mut record =
                    self.next_event_base(MC_EVENT_LEVEL1_CASTLE_PROJECTILE_SPAWNED, hash);
                record.entity_index = projectile_entity_index;
                record.code = u32::from(spell_family)
                    | (u32::from(subspell) << 8)
                    | (u32::from(first_shot) << 16);
                record.value_0 = i32::from(target_entity_index);
                record.value_1 = i32::from(controller_entity_index);
                record.reserved[0] = u32::from(yaw.cast_unsigned());
                record.reserved[1] = u32::from(pitch.cast_unsigned());
                record
            }
            Level1MissionEvent::CastleManaChanged { mana } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_CASTLE_MANA_CHANGED, hash);
                record.value_0 = mana;
                record
            }
            Level1MissionEvent::PlayerDamaged { damage, life } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_PLAYER_DAMAGED, hash);
                record.entity_index = self.state.entity_index;
                record.value_0 = damage;
                record.value_1 = life;
                record
            }
            Level1MissionEvent::PlayerDied => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_PLAYER_DIED, hash);
                record.entity_index = self.state.entity_index;
                record
            }
            Level1MissionEvent::MissionSound {
                sound_id,
                source_entity_index,
                position,
                random_state,
            } => {
                let mut record = self.next_event_base(MC_EVENT_PREPARE_SOUND, hash);
                record.code = u32::from(sound_id);
                let source_action = self
                    .mission
                    .source_slots()
                    .get(usize::from(source_entity_index))
                    .map_or(0, |source| source.action);
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    source_entity_index,
                    position,
                    source_action,
                    random_state,
                );
                record
            }
            Level1MissionEvent::ExitSequenceStarted => {
                self.next_event_base(MC_EVENT_LEVEL1_EXIT_SEQUENCE_STARTED, hash)
            }
            Level1MissionEvent::ExitSequenceCompleted { result_code } => {
                let mut record =
                    self.next_event_base(MC_EVENT_LEVEL1_EXIT_SEQUENCE_COMPLETED, hash);
                record.code = u32::from(result_code);
                record
            }
            Level1MissionEvent::LevelCompleted => {
                self.next_event_base(MC_EVENT_LEVEL1_COMPLETED, hash)
            }
        }
    }

    pub(crate) fn attach_mission_sound(
        &mut self,
        record: &mut McEventV2,
        sound_id: u16,
        source_entity_index: u16,
        source_position: Position,
        source_action: u8,
        random_state: u32,
    ) {
        record.entity_index = source_entity_index;
        record.asset_ref_index = self
            .world
            .sound_asset_ref(sound_id)
            .unwrap_or(MC_ASSET_REF_NONE);
        let source_id = self
            .world
            .entities()
            .get(usize::from(source_entity_index))
            .map_or(source_entity_index, |source| source.source_id);
        if let Some(prepared) = prepare_event_sound_command(
            self.state.position,
            self.state.yaw,
            source_position,
            sound_id,
            random_state,
            source_id,
            source_action,
        ) {
            self.insert_prepared_audio_command(
                record.sequence,
                audio_command_record(
                    record.generation,
                    record.sequence,
                    record.state_hash,
                    record.asset_ref_index,
                    sound_id,
                    source_entity_index,
                    record.player_index,
                    prepared.command,
                    prepared.play_type,
                ),
            );
        }
    }

    pub(crate) fn prepare_audio_command(&self, event: &McEventV2) -> Option<McAudioCommandV2> {
        let sound_id = u16::try_from(event.code).ok()?;
        if matches!(sound_id, 1 | 2 | 5 | 31) {
            return Some(local_audio_command_record(
                event,
                sound_id,
                0,
                POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING,
            ));
        }
        if matches!(sound_id, 14 | 29) {
            // PrepareEventSound's local LevelIndex/-1 branch stores type 1
            // for menu select and can't-use feedback.
            return Some(local_audio_command_record(event, sound_id, 0, 1));
        }
        let source = self.world.entities().get(usize::from(event.entity_index))?;
        let source_position = if event.entity_index == self.state.entity_index {
            self.state.position
        } else {
            source.position
        };
        let random_state = self
            .context
            .effects
            .as_ref()
            .map_or(self.world.runtime().random_state, |effects| {
                effects.global_rng
            });
        let prepared = prepare_event_sound_command(
            self.state.position,
            self.state.yaw,
            source_position,
            sound_id,
            random_state,
            source.source_id,
            source.action,
        )?;
        Some(audio_command_record(
            event.generation,
            event.sequence,
            event.state_hash,
            event.asset_ref_index,
            sound_id,
            event.entity_index,
            event.player_index,
            prepared.command,
            prepared.play_type,
        ))
    }
}

pub(super) fn selector_state_hash_v1(frames: &GoatTmapClocks, feedback: Option<(u64, u8)>) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    for byte in b"mc2-selector-frame-session-v1\0"
        .iter()
        .copied()
        .chain(frames.hash_v1().to_le_bytes())
        .chain(u8::from(feedback.is_some()).to_le_bytes())
        .chain(feedback.map_or(0, |value| value.0).to_le_bytes())
        .chain(feedback.map_or(0, |value| value.1).to_le_bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

pub(super) fn camera_history_hash(camera: CameraHistory) -> u64 {
    let mut bytes = Vec::with_capacity(64);
    bytes.extend_from_slice(b"mc2-presentation-camera-v1\0");
    bytes.extend_from_slice(&camera.tick.to_le_bytes());
    bytes.extend_from_slice(&camera.source_turn.to_le_bytes());
    bytes.extend_from_slice(&camera.x.to_le_bytes());
    bytes.extend_from_slice(&camera.y.to_le_bytes());
    bytes.extend_from_slice(&camera.z.to_le_bytes());
    bytes.extend_from_slice(&camera.yaw.to_le_bytes());
    bytes.extend_from_slice(&camera.pitch.to_le_bytes());
    bytes.extend_from_slice(&camera.roll.to_le_bytes());
    bytes.extend_from_slice(&camera.fov.to_le_bytes());
    sim_core::presentation_frame::byte_hash(&bytes)
}

pub(super) fn selector_frame_state(
    ordinal: usize,
    clock: &GoatTmapClock,
    bank: &GoatTmapBank,
) -> McSelectorFrameStateV2 {
    let selector = &bank.selectors()[ordinal];
    McSelectorFrameStateV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McSelectorFrameStateV2>(),
        selector: clock.selector(),
        mask_bit: ordinal as u8,
        authority: MC_AUTHORITY_SELECTOR_FRAME_LIVE,
        width: clock.width(),
        height: clock.height(),
        source_frame_index: u16::from(clock.frame_index()),
        frame_count: 4,
        stream_start: selector.stream_start,
        stream_cursor: clock.stream_cursor(),
        payload_bytes: selector.payload_bytes,
        pixel_bytes: u32::try_from(clock.pixels().len())
            .expect("qualified Goat TMAP pixel count fits u32"),
        flags: (MC_SELECTOR_FRAME_ANIMATED | MC_SELECTOR_FRAME_PIXELS_MUTABLE)
            & !MC_SELECTOR_FRAME_GROUPED_UPDATE
            | if clock.used_by_preceding_draw() {
                MC_SELECTOR_FRAME_USED_BY_PRECEDING_DRAW
            } else {
                0
            },
        pixel_hash: clock.pixel_hash_v1(),
        reserved: [0; 4],
    }
}

pub(super) fn local_audio_command_record(
    event: &McEventV2,
    sound_id: u16,
    sound_flags: i16,
    play_type: u8,
) -> McAudioCommandV2 {
    let loop_count = if matches!(sound_id, 1 | 2 | 5 | 31) {
        -1
    } else {
        0
    };
    McAudioCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McAudioCommandV2>(),
        generation: event.generation,
        event_sequence: event.sequence,
        state_hash: event.state_hash,
        asset_ref_index: event.asset_ref_index,
        playback_rate_hz: 22_050,
        sound_id,
        entity_index: event.entity_index,
        player_index: event.player_index,
        sound_flags,
        volume_q15: if matches!(sound_id, 14 | 29) {
            0x7fff
        } else {
            0
        },
        pan_q16: 0x7fff,
        voice_index: MC_AUDIO_VOICE_NONE,
        play_type,
        loop_count,
        action: 0,
        reserved: [0; 4],
        ..McAudioCommandV2::default()
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn audio_command_record(
    generation: u64,
    event_sequence: u64,
    state_hash: u64,
    asset_ref_index: u32,
    sound_id: u16,
    entity_index: u16,
    player_index: i16,
    command: PositionalSoundCommand,
    play_type: u8,
) -> McAudioCommandV2 {
    let playback_rate_hz = if command.audible {
        let percent = u32::try_from(100 + i32::from(command.rate_delta_percent))
            .expect("bounded playback percentage is positive");
        22_050 * percent / 100
    } else {
        0
    };
    McAudioCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McAudioCommandV2>(),
        generation,
        event_sequence,
        state_hash,
        asset_ref_index,
        relative_x: command.relative_x,
        relative_y: command.relative_y,
        relative_z: command.relative_z,
        playback_rate_hz,
        sound_id,
        entity_index,
        player_index,
        sound_flags: command.sound_flags,
        volume_q15: command.volume_q15,
        pan_q16: command.pan_q16,
        rate_delta_percent: command.rate_delta_percent,
        voice_index: MC_AUDIO_VOICE_NONE,
        play_type,
        loop_count: 0,
        action: if command.audible {
            0
        } else {
            MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME
        },
        pooled_flush_rank: 0,
        reserved: [0; 4],
    }
}

pub(super) fn positional_qualification_command(index: u32) -> AbiResult<McAudioCommandV2> {
    let (dx, dy, sound_id, random_state) = match index {
        0 | 22 => (0, -1_024, 43, 28),
        1 => (512, -512, 37, 0),
        2 => (1_024, 0, 39, 0),
        3 => (512, 512, 37, 0),
        4 => (0, 1_024, 37, 0),
        5 => (-512, 512, 37, 0),
        6 => (-1_024, 0, 37, 0),
        7 => (-512, -512, 37, 0),
        8 => (320, 0, 37, 0),
        9 => (321, 0, 37, 0),
        10 => (0, -12_095, 37, 0),
        11 => (0, -12_096, 37, 0),
        12 => (9_071, 0, 37, 0),
        13 => (9_072, 0, 37, 0),
        14 => (0, 6_047, 37, 0),
        15 => (0, 6_048, 37, 0),
        16 => (12_288, 0, 37, 0),
        17 => (12_288, 1, 37, 0),
        18 => (0, 0, 43, 13),
        19 => (0, 0, 43, 28),
        20 => (0, 0, 43, 20),
        21 => (0, -6_000, 43, 28),
        _ => {
            return Err(AbiError::new(
                MC_STATUS_BOUNDS,
                format!("positional qualification vector {index} is out of bounds"),
            ));
        }
    };
    let listener = Position {
        x: 32_768,
        y: 32_768,
        z: 1_000,
    };
    let source = Position {
        x: u16::try_from(i32::from(listener.x) + dx).expect("qualification x remains in range"),
        y: u16::try_from(i32::from(listener.y) + dy).expect("qualification y remains in range"),
        z: listener.z,
    };
    let prepared = prepare_event_sound_command(listener, 0, source, sound_id, random_state, 321, 0)
        .expect("qualification vector uses a proven Level 1 positional sound");
    let asset_ref_index = match sound_id {
        37 => 95,
        39 => 96,
        43 => 99,
        _ => unreachable!("qualification sound is bounded"),
    };
    let mut record = audio_command_record(
        1,
        u64::from(index) + 1,
        0,
        asset_ref_index,
        sound_id,
        321,
        -1,
        prepared.command,
        prepared.play_type,
    );
    if record.action == 0 {
        record.action = MC_AUDIO_ACTION_START;
        record.voice_index = 0;
    }
    if index == 22 {
        record.action = MC_AUDIO_ACTION_SUPPRESS_EXISTING;
        record.voice_index = 0;
    }
    Ok(record)
}
