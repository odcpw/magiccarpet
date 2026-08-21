//! Level 1 lifecycle, presentation, world, and qualification ABI exports.
//!
//! This family is a pure move from the native_v2 facade; the exported
//! signatures and no-mangle symbol identities remain unchanged.

use super::*;

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_mission_snapshot(
    handle: u64,
    out_snapshot: *mut McLevel1MissionSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| Ok(level1_mission_snapshot(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_player_lifecycle_snapshot(
    handle: u64,
    out_snapshot: *mut McLevel1PlayerLifecycleSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| {
            Ok(level1_player_lifecycle_snapshot(session))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_player_menu_state(handle: u64, menu_state: u8) -> i32 {
    boundary(|| {
        with_session_mut(handle, |session| {
            let current = session.mission.player_menu_state;
            session.mission.set_player_menu_state(menu_state);
            if current != menu_state && menu_state != 8 && !matches!(current, 5 | 8) {
                session.emit_local_flight_sound(sim_core::menu_cursor_transition_audio_event(
                    session.state.entity_index,
                    i16::from(session.world.runtime().local_player_slot),
                ));
            }
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_game_paused(handle: u64, paused: u8) -> i32 {
    boundary(|| {
        if paused > 1 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "Level 1 paused state must be zero or one",
            ));
        }
        with_session_mut(handle, |session| {
            session.set_game_paused(paused != 0);
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_presentation_input(
    handle: u64,
    input: *const McLevel1PresentationInputV2,
) -> i32 {
    boundary(|| {
        let input = read_versioned_input(input, "Level 1 presentation input")?;
        with_session_mut(handle, |session| session.apply_presentation_input(input))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_player_inject_damage_for_qualification(
    handle: u64,
    damage: i32,
    attacker_entity_index: u16,
) -> i32 {
    boundary(|| {
        with_session_mut(handle, |session| {
            session
                .mission
                .submit_player_damage(damage, attacker_entity_index);
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_action_51_blocker_setup_for_qualification(
    handle: u64,
    house_entity_index: u16,
    house_action: u8,
) -> i32 {
    boundary(|| {
        if !matches!(
            house_action,
            sim_core::house::HOUSE_TERRAIN_TO_LIVE_ACTION | u8::MAX
        ) {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "action-51 qualification house action must be 51 or 255",
            ));
        }
        with_session_mut(handle, |session| {
            let mut mission = session.mission.clone();
            let mut live_world = session.live_world.clone();
            let mut house = live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == house_entity_index)
                .copied()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!("action-51 qualification house {house_entity_index} is absent"),
                    )
                })?;
            house.action = house_action;
            house.max_life = 2;
            house.life = 2;
            house.owner_entity_index = session.state.entity_index;
            house.control_flags |= 1;
            live_world
                .initialize_builder_house(house)
                .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
            mission
                .inject_action_51_blocker_for_qualification(
                    house.position,
                    house.collision_half_width,
                    house.collision_half_depth,
                )
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "action-51 qualification blocker allocation failed",
                    )
                })?;
            session.mission = mission;
            session.live_world = live_world;
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_action_51_completion_setup_for_qualification(
    handle: u64,
    house_entity_index: u16,
    parent_entity_index: u16,
    construction_selector: u8,
) -> i32 {
    boundary(|| {
        with_session_mut(handle, |session| {
            if session
                .world
                .house_build_footprint(construction_selector)
                .is_none()
            {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("action-51 qualification BUILD {construction_selector} is absent"),
                ));
            }
            let mut mission = session.mission.clone();
            let mut live_world = session.live_world.clone();
            let views = level1_turn_start_source_views(&mission, &session.state, &live_world);
            let parent = views
                .iter()
                .copied()
                .find(|view| view.source_entity_index == parent_entity_index)
                .filter(|view| view.class == 3 && view.life >= 0)
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!(
                            "action-51 qualification parent {parent_entity_index} is not a live class-3 row"
                        ),
                    )
                })?;
            let construction_axis = Position {
                x: parent.position.x.wrapping_add(0x0101),
                y: parent.position.y.wrapping_add(0x0202),
                z: parent.position.z.wrapping_add(33),
            };
            if !mission.configure_action_51_completion_parent_for_qualification(
                parent_entity_index,
                construction_selector,
                construction_axis,
            ) {
                return Err(AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "action-51 qualification parent source fields could not be configured",
                ));
            }
            let mut house = live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == house_entity_index)
                .copied()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!("action-51 qualification house {house_entity_index} is absent"),
                    )
                })?;
            house.action = sim_core::house::HOUSE_TERRAIN_TO_LIVE_ACTION;
            house.max_life = 1;
            house.life = 1;
            house.owner_entity_index = session.state.entity_index;
            house.control_flags |= 1;
            house.position = parent.position;
            live_world
                .initialize_builder_house(house)
                .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
            session.mission = mission;
            session.live_world = live_world;
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_hud_snapshot(
    handle: u64,
    out_snapshot: *mut McLevel1HudSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| Ok(level1_hud_snapshot(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_spell_menu_snapshot(
    handle: u64,
    out_snapshot: *mut McLevel1SpellMenuSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| Ok(level1_spell_menu_snapshot(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_minimap_marks(
    handle: u64,
    out_state: *mut McLevel1MinimapMarksV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_session(handle, |session| Ok(level1_minimap_marks(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_minimap_entities_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    buffer: *mut McLevel1MinimapEntityCommandV2,
    capacity: u32,
    element_size: u32,
    out_info: *mut McLevel1MinimapEntitiesInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        require_element_size::<McLevel1MinimapEntityCommandV2>(
            element_size,
            "Level 1 minimap entity command",
        )?;
        with_session(handle, |session| {
            validate_mission_snapshot_cursor(session, generation, expected_tick)?;
            let commands = level1_minimap_entity_commands(session);
            let capacity = usize::try_from(capacity).map_err(|_| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    "Level 1 minimap command capacity exceeds usize",
                )
            })?;
            if capacity < commands.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "Level 1 minimap command capacity is smaller than the atomic stream",
                ));
            }
            if !commands.is_empty() && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "Level 1 minimap command buffer is null",
                ));
            }
            for (offset, command) in commands.iter().enumerate() {
                // SAFETY: element size and caller capacity were validated.
                unsafe {
                    buffer.add(offset).write_unaligned(*command);
                }
            }
            Ok(McLevel1MinimapEntitiesInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McLevel1MinimapEntitiesInfoV2>(),
                generation: session.generation,
                tick: session.state.tick,
                total_count: u32::try_from(commands.len())
                    .expect("Level 1 minimap command count fits u32"),
                cross_x: 63,
                cross_y: 64,
                cross_colour: 176,
                cross_arm_length: 10,
                reveal_active: 0,
                authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
                reserved_0: [0; 4],
                command_hash: level1_minimap_command_hash(&commands),
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_palette_copy(
    handle: u64,
    rgb8_buffer: *mut u8,
    rgb8_capacity: u32,
    out_info: *mut McLevel1PaletteInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        with_session(handle, |session| {
            if rgb8_buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "Level 1 effective palette buffer must be non-null",
                ));
            }
            if rgb8_capacity < 768 {
                return Err(AbiError::new(
                    MC_STATUS_BUFFER_TOO_SMALL,
                    format!("Level 1 effective palette requires 768 bytes, got {rgb8_capacity}"),
                ));
            }
            let rgb8 = session.level1_palette_rgb8()?;
            // SAFETY: the caller supplied a non-null buffer with at least the
            // checked 768-byte capacity.
            unsafe {
                rgb8_buffer.copy_from_nonoverlapping(rgb8.as_ptr(), rgb8.len());
            }
            let digest = Sha256::digest(rgb8);
            let mut rgb8_sha256 = [0_u8; 32];
            rgb8_sha256.copy_from_slice(&digest);
            Ok(McLevel1PaletteInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McLevel1PaletteInfoV2>(),
                generation: session.generation,
                tick: session.state.tick,
                submode: session.mission.exit_palette_submode,
                effect: session.mission.exit_palette_effect,
                weight: session.mission.exit_palette_weight,
                denominator: session.mission.exit_palette_denominator,
                palette_count: session.mission.palette_count(),
                flags: 1,
                rgb8_sha256,
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_exit_presentation(
    handle: u64,
    out_state: *mut McLevel1ExitPresentationV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_session(handle, |session| Ok(level1_exit_presentation(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_castle_snapshot(
    handle: u64,
    out_snapshot: *mut McLevel1CastleSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| Ok(level1_castle_snapshot(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_castle_lifecycle_snapshot(
    handle: u64,
    out_snapshot: *mut McLevel1CastleLifecycleSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| {
            Ok(level1_castle_lifecycle_snapshot(session))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_house_completion_effect_count(
    handle: u64,
    out_count: *mut u32,
) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_session(handle, |session| {
            Ok(
                u32::try_from(level1_house_completion_effects(session).len())
                    .expect("completion effect count fits u32"),
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_house_completion_effect_snapshot(
    handle: u64,
    effect_ordinal: u32,
    out_snapshot: *mut McLevel1HouseCompletionEffectV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| {
            level1_house_completion_effects(session)
                .get(usize::try_from(effect_ordinal).expect("u32 ordinal fits usize"))
                .copied()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!(
                            "Level 1 house-completion effect ordinal {effect_ordinal} is out of bounds"
                        ),
                    )
                })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_mana_carrier_snapshot(
    handle: u64,
    out_snapshot: *mut McManaCarrierSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| Ok(mana_carrier_snapshot(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_mana_spheres_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    start_ordinal: u32,
    buffer: *mut McManaSphereSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McManaSphereSnapshotV2>(element_size, "mana sphere element")?;
        with_session(handle, |session| {
            validate_mission_snapshot_cursor(session, generation, expected_tick)?;
            copy_mission_slice(
                session.mission.mana_spheres(),
                start_ordinal,
                buffer,
                capacity,
                out_written,
                mana_sphere_snapshot,
                "mana sphere",
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_actors_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    start_ordinal: u32,
    buffer: *mut McLevel1ActorSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McLevel1ActorSnapshotV2>(element_size, "Level 1 actor element")?;
        with_session(handle, |session| {
            validate_mission_snapshot_cursor(session, generation, expected_tick)?;
            copy_mission_slice(
                session.mission.actors(),
                start_ordinal,
                buffer,
                capacity,
                out_written,
                level1_actor_snapshot,
                "Level 1 actor",
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_multipart_segments_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    start_ordinal: u32,
    buffer: *mut McLevel1MultipartSegmentSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McLevel1MultipartSegmentSnapshotV2>(
            element_size,
            "Level 1 multipart segment element",
        )?;
        with_session(handle, |session| {
            validate_mission_snapshot_cursor(session, generation, expected_tick)?;
            copy_mission_slice(
                session.mission.multipart_segments(),
                start_ordinal,
                buffer,
                capacity,
                out_written,
                level1_multipart_segment_snapshot,
                "Level 1 multipart segment",
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_fireball_projectiles_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    start_ordinal: u32,
    buffer: *mut McFireballProjectileSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McFireballProjectileSnapshotV2>(
            element_size,
            "Fireball projectile element",
        )?;
        with_session(handle, |session| {
            validate_mission_snapshot_cursor(session, generation, expected_tick)?;
            copy_mission_slice(
                session.mission.projectiles(),
                start_ordinal,
                buffer,
                capacity,
                out_written,
                fireball_projectile_snapshot,
                "Fireball projectile",
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_fireball_impacts_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    start_ordinal: u32,
    buffer: *mut McFireballImpactSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McFireballImpactSnapshotV2>(
            element_size,
            "Fireball impact element",
        )?;
        with_session(handle, |session| {
            validate_mission_snapshot_cursor(session, generation, expected_tick)?;
            copy_mission_slice(
                session.mission.impacts(),
                start_ordinal,
                buffer,
                capacity,
                out_written,
                fireball_impact_snapshot,
                "Fireball impact",
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_selector_frame_states_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    start_ordinal: u32,
    buffer: *mut McSelectorFrameStateV2,
    capacity: u32,
    element_size: u32,
    out_batch: *mut McSelectorFrameBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        require_element_size::<McSelectorFrameStateV2>(element_size, "selector-frame element")?;
        with_session(handle, |session| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "selector-frame generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            if expected_tick != session.state.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "selector-frame tick {expected_tick} does not match {}",
                        session.state.tick
                    ),
                ));
            }
            let frames = session.selector_frames.as_ref().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "selector-frame state unavailable without qualified owner cache",
                )
            })?;
            let bank = session.world.goat_tmap_bank().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "selector-frame bank unavailable without qualified owner cache",
                )
            })?;
            let start = usize::try_from(start_ordinal).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "selector-frame start exceeds usize")
            })?;
            if start > GOAT_TMAP_SELECTORS.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "selector-frame start is beyond selector count",
                ));
            }
            let capacity = usize::try_from(capacity).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "selector-frame capacity exceeds usize")
            })?;
            let count = (GOAT_TMAP_SELECTORS.len() - start).min(capacity);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "selector-frame output buffer is null",
                ));
            }
            for (offset, clock) in frames.states().iter().skip(start).take(count).enumerate() {
                // SAFETY: exact element size and writable capacity were
                // supplied for `count` rows.
                unsafe {
                    buffer.add(offset).write_unaligned(selector_frame_state(
                        start + offset,
                        clock,
                        bank,
                    ));
                }
            }
            session.selector_frame_batch(
                start_ordinal,
                u32::try_from(count).expect("three selector frames fit u32"),
            )
        })
    })
}

/// Run the qualified original CPU Goat draw-admission pass for the completed
/// session tick and OR its three-bit result into the selector used latches.
///
/// This function consumes only authoritative Rust simulation, camera-history,
/// terrain-lattice, and owner-cache state.  Managed/GPU visibility is not an
/// input.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_goat_draw_admission_evaluate(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    out_batch: *mut McSelectorFrameBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        with_session_mut(handle, |session| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "Goat draw-admission generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            if expected_tick != session.state.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "Goat draw-admission tick {expected_tick} does not match {}",
                        session.state.tick
                    ),
                ));
            }
            session.evaluate_goat_draw_admission()
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_selector_draw_feedback_submit(
    handle: u64,
    command: *const McSelectorDrawFeedbackV2,
    out_batch: *mut McSelectorFrameBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        let command = read_versioned_input(command, "selector draw feedback")?;
        validate_reserved(&command.reserved, "selector draw feedback")?;
        if command.selector_mask & !u64::from(GOAT_TMAP_DRAW_MASK) != 0 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                format!(
                    "selector draw mask {:#x} has unsupported bits",
                    command.selector_mask
                ),
            ));
        }
        with_session_mut(handle, |session| {
            if command.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "selector draw generation {} does not match {}",
                        command.generation, session.generation
                    ),
                ));
            }
            if command.draw_tick != session.state.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "selector draw tick {} does not match {}",
                        command.draw_tick, session.state.tick
                    ),
                ));
            }
            let mask =
                u8::try_from(command.selector_mask).expect("validated selector draw mask fits u8");
            if let Some((tick, prior_mask)) = session.selector_draw_feedback
                && tick == command.draw_tick
            {
                let combined_mask = prior_mask | mask;
                if combined_mask == prior_mask {
                    let mut receipt = session.selector_frame_batch(0, 0)?;
                    receipt.has_more = 0;
                    return Ok(receipt);
                }
                let frames = session.selector_frames.as_mut().ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "selector-frame state unavailable without qualified owner cache",
                    )
                })?;
                frames
                    .latch_draw_attempts(mask)
                    .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error))?;
                session.selector_draw_feedback = Some((command.draw_tick, combined_mask));
                let mut receipt = session.selector_frame_batch(0, 0)?;
                receipt.has_more = 0;
                return Ok(receipt);
            }
            let frames = session.selector_frames.as_mut().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "selector-frame state unavailable without qualified owner cache",
                )
            })?;
            frames
                .latch_draw_attempts(mask)
                .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error))?;
            session.selector_draw_feedback = Some((command.draw_tick, mask));
            let mut receipt = session.selector_frame_batch(0, 0)?;
            receipt.has_more = 0;
            Ok(receipt)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_goat_inject_damage(
    handle: u64,
    command: *const McGoatDamageCommandV2,
    out_batch: *mut McGoatBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        let command = read_versioned_input(command, "Goat damage command")?;
        validate_reserved(&command.reserved, "Goat damage command")?;
        if command.reserved_0 != 0 {
            return Err(AbiError::new(
                MC_STATUS_RESERVED_NONZERO,
                "Goat damage command reserved_0 must be zero",
            ));
        }
        with_session_mut(handle, |session| {
            if command.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "Goat damage generation {} does not match {}",
                        command.generation, session.generation
                    ),
                ));
            }
            session
                .live_world
                .inject_damage(
                    command.entity_index,
                    command.damage,
                    session.state.entity_index.cast_signed(),
                )
                .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error))?;
            Ok(McGoatBatchV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McGoatBatchV2>(),
                generation: session.generation,
                tick: session.live_world.outer_tick().into(),
                live_world_hash: session.live_world_hash_v2(),
                start_ordinal: 0,
                written: 0,
                total_count: u32::try_from(LEVEL_ONE_GOAT_INDICES.len()).expect("33 Goats fit u32"),
                has_more: 0,
                reserved_0: [0; 3],
                reserved: [0; 4],
            })
        })
    })
}
