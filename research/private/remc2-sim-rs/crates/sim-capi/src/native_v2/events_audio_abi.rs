//! Asset references, event paging, audio lifecycle, and error ABI exports.
//!
//! This family is a pure move from the native_v2 facade; exported names,
//! signatures, and no-mangle linkage remain unchanged.

use super::*;

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_asset_ref_count(handle: u64, out_count: *mut u32) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_session(handle, |session| {
            Ok(u32::try_from(session.world.asset_refs().len())
                .expect("validated asset count fits u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_asset_ref_info(
    handle: u64,
    asset_index: u32,
    out_info: *mut McAssetRefInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        with_session(handle, |session| {
            let asset = asset_ref(&session.world, asset_index)?;
            Ok(McAssetRefInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McAssetRefInfoV2>(),
                asset_index,
                trace_ordinal: asset.observation.ordinal,
                owner_asset_count: u16::try_from(asset.owner_asset_ids.len()).map_err(|_| {
                    AbiError::new(MC_STATUS_BOUNDS, "owner asset count exceeds u16")
                })?,
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_asset_string_length(
    handle: u64,
    asset_index: u32,
    kind: u32,
    owner_index: u32,
    out_length: *mut u32,
) -> i32 {
    boundary_value(out_length, struct_size::<u32>(), || {
        with_session(handle, |session| {
            let asset = asset_ref(&session.world, asset_index)?;
            string_length(asset_string(asset, kind, owner_index)?)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_asset_string_copy(
    handle: u64,
    asset_index: u32,
    kind: u32,
    owner_index: u32,
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_length, "out_length")?;
        with_session(handle, |session| {
            let asset = asset_ref(&session.world, asset_index)?;
            copy_string(
                asset_string(asset, kind, owner_index)?,
                buffer,
                capacity,
                out_length,
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_events_copy_since(
    handle: u64,
    query: *const McEventQueryV2,
    buffer: *mut McEventV2,
    capacity: u32,
    element_size: u32,
    out_batch: *mut McEventBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        require_element_size::<McEventV2>(element_size, "event element")?;
        let query = read_versioned_input(query, "event query")?;
        validate_reserved(&query.reserved, "event query")?;
        with_session(handle, |session| {
            if query.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "event query generation {} does not match {}",
                        query.generation, session.generation
                    ),
                ));
            }
            if query.cursor > session.event_head {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "event cursor is beyond current event head",
                ));
            }
            if let Some(oldest) = session.events.front()
                && query.cursor.saturating_add(1) < oldest.sequence
            {
                return Err(AbiError::new(
                    MC_STATUS_CURSOR_EXPIRED,
                    "event cursor predates the retained journal",
                ));
            }
            let available = session
                .events
                .iter()
                .filter(|event| event.sequence > query.cursor)
                .collect::<Vec<_>>();
            let count = available.len().min(
                usize::try_from(capacity)
                    .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "capacity does not fit usize"))?,
            );
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "event output buffer is null",
                ));
            }
            for (index, event) in available.iter().take(count).enumerate() {
                // SAFETY: buffer is non-null for count records and the caller
                // promises capacity writable records.
                unsafe { buffer.add(index).write_unaligned(**event) };
            }
            let next_cursor = if count == 0 {
                query.cursor
            } else {
                available[count - 1].sequence
            };
            Ok(McEventBatchV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McEventBatchV2>(),
                generation: session.generation,
                next_cursor,
                event_head: session.event_head,
                written: u32::try_from(count).expect("event batch count fits u32"),
                has_more: u8::from(count < available.len()),
                reserved_0: [0; 3],
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_audio_command_for_event(
    handle: u64,
    generation: u64,
    event_sequence: u64,
    out_command: *mut McAudioCommandV2,
    out_command_size: u32,
) -> i32 {
    boundary_value(out_command, out_command_size, || {
        with_session(handle, |session| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "audio command generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            session
                .audio_commands
                .get(&event_sequence)
                .copied()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!("event sequence {event_sequence} has no positional audio command"),
                    )
                })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_audio_command_release(
    handle: u64,
    generation: u64,
    event_sequence: u64,
) -> i32 {
    boundary(|| {
        with_session_mut(handle, |session| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "audio release generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let command = session.audio_commands.get(&event_sequence).ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    format!("event sequence {event_sequence} has no positional audio command"),
                )
            })?;
            if command.action == 0 {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("event sequence {event_sequence} is still pending"),
                ));
            }
            session.audio_commands.remove(&event_sequence);
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_audio_command_discard(
    handle: u64,
    generation: u64,
    event_sequence: u64,
) -> i32 {
    boundary(|| {
        with_session_mut(handle, |session| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "audio discard generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let command = session
                .audio_commands
                .get(&event_sequence)
                .copied()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!("event sequence {event_sequence} has no positional audio command"),
                    )
                })?;
            if command.action == MC_AUDIO_ACTION_PREPARED_PENDING {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("event sequence {event_sequence} is still pending"),
                ));
            }
            if matches!(
                command.action,
                MC_AUDIO_ACTION_START
                    | MC_AUDIO_ACTION_RESTART
                    | MC_AUDIO_ACTION_UPDATE_EXISTING
                    | MC_AUDIO_ACTION_END_LOOP
            ) {
                let index = usize::from(command.voice_index);
                let voice = session.audio_voices.get_mut(index).ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!("audio voice {} is out of bounds", command.voice_index),
                    )
                })?;
                let owns_voice = voice.is_some_and(|voice| {
                    voice.sound_id == command.sound_id
                        && voice.sound_flags == command.sound_flags
                        && if matches!(
                            command.action,
                            MC_AUDIO_ACTION_UPDATE_EXISTING | MC_AUDIO_ACTION_END_LOOP
                        ) {
                            voice.event_sequence <= command.event_sequence
                        } else {
                            voice.event_sequence == command.event_sequence
                        }
                });
                if owns_voice {
                    *voice = None;
                }
            }
            session.audio_commands.remove(&event_sequence);
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_audio_voice_complete(
    handle: u64,
    generation: u64,
    voice_index: u16,
    event_sequence: u64,
) -> i32 {
    boundary(|| {
        with_session_mut(handle, |session| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "audio completion generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let index = usize::from(voice_index);
            let voice = session.audio_voices.get_mut(index).ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    format!("audio voice {voice_index} is out of bounds"),
                )
            })?;
            if !voice.is_some_and(|voice| voice.event_sequence == event_sequence) {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    format!("audio voice {voice_index} does not own event {event_sequence}"),
                ));
            }
            *voice = None;
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub const extern "C" fn mc_native_audio_qualification_vector_count() -> u32 {
    POSITIONAL_QUALIFICATION_VECTOR_COUNT
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_audio_qualification_vector(
    index: u32,
    out_command: *mut McAudioCommandV2,
    out_command_size: u32,
) -> i32 {
    boundary_value(out_command, out_command_size, || {
        positional_qualification_command(index)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_audio_qualification_submit(
    handle: u64,
    event_kind: u32,
    sound_id: u16,
    sound_flags: i16,
    volume_q15: u16,
    pan_q16: u16,
    play_type: u8,
    out_event: *mut McEventV2,
    out_event_size: u32,
    out_command: *mut McAudioCommandV2,
    out_command_size: u32,
) -> i32 {
    boundary(|| {
        require_struct_output::<McEventV2>(out_event, out_event_size, "audio event")?;
        require_struct_output::<McAudioCommandV2>(out_command, out_command_size, "audio command")?;
        if !matches!(event_kind, MC_EVENT_PREPARE_SOUND | MC_EVENT_END_LOOP)
            || !matches!(sound_id, 1 | 2 | 5 | 29 | 31 | 37 | 39 | 43)
            || !matches!(play_type, POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING | 3)
            || volume_q15 > 0x7fff
        {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "qualification audio submission is outside the proven effect contract",
            ));
        }
        let (event, command) = with_session_mut(handle, |session| {
            let mut event = session.next_event_base(event_kind, state_hash(&session.state));
            event.code = u32::from(sound_id);
            event.entity_index = session.state.entity_index;
            event.player_index = -1;
            event.asset_ref_index = session.world.sound_asset_ref(sound_id).ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    format!("sound {sound_id} has no qualified owner asset"),
                )
            })?;
            let mut command = local_audio_command_record(&event, sound_id, sound_flags, play_type);
            command.volume_q15 = volume_q15;
            command.pan_q16 = pan_q16;
            if event_kind == MC_EVENT_END_LOOP {
                let matching = session.audio_voices.iter().position(|voice| {
                    voice.is_some_and(|voice| {
                        voice.sound_id == sound_id && voice.sound_flags == sound_flags
                    })
                });
                if let Some(index) = matching {
                    command.action = MC_AUDIO_ACTION_END_LOOP;
                    command.voice_index = u16::try_from(index).expect("ten voices fit u16");
                } else {
                    command.action = MC_AUDIO_ACTION_SUPPRESS_NO_EXISTING;
                }
            } else {
                let (action, voice_index) = session.allocate_audio_voice(command);
                command.action = if matches!(sound_id, 1 | 2 | 5 | 31)
                    && action == MC_AUDIO_ACTION_SUPPRESS_EXISTING
                {
                    MC_AUDIO_ACTION_UPDATE_EXISTING
                } else {
                    action
                };
                command.voice_index = voice_index;
            }
            session.audio_commands.insert(event.sequence, command);
            session.append_event(event);
            Ok((event, command))
        })?;
        write_output(out_event, event);
        write_output(out_command, command);
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_last_error_length(out_length: *mut u32) -> i32 {
    match catch_unwind(AssertUnwindSafe(|| {
        require_output(out_length, "out_length")?;
        LAST_ERROR.with(|error| {
            let length = string_length(&error.borrow())?;
            write_output(out_length, length);
            Ok::<(), AbiError>(())
        })
    })) {
        Ok(Ok(())) => MC_STATUS_OK,
        Ok(Err(error)) => {
            set_last_error(&error.message);
            error.status
        }
        Err(_) => MC_STATUS_INTERNAL_PANIC,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_last_error_copy(
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    match catch_unwind(AssertUnwindSafe(|| {
        let message = LAST_ERROR.with(|error| error.borrow().clone());
        require_output(out_length, "out_length")?;
        copy_string(&message, buffer, capacity, out_length)
    })) {
        Ok(Ok(())) => MC_STATUS_OK,
        // Error retrieval must not destroy the diagnostic being retrieved.
        Ok(Err(error)) => error.status,
        Err(_) => MC_STATUS_INTERNAL_PANIC,
    }
}
