//! Neutral, definition-bound session ABI.
//!
//! The legacy v2 ABI remains available for same-build compatibility
//! comparison. These additive exports own a distinct registry containing the
//! permanent `sim-core` `GameSession`; JSON is a bounded transport encoding of
//! domain values, never the authority or a client-side gameplay model.

use super::campaign_facade::SealedLevel1Backend;
use super::*;
use sim_core::{
    game_definition::{
        GameDefinition, Mc2OwnedInputs, WorkspaceStore, default_catalog, load_mc2_owned,
    },
    game_session::{
        GameCommand, GameSession, GameSessionAssetError, GameSessionError, OpenSessionError,
        SessionGuard, SessionGuardErrorCode,
    },
};
pub(super) type NeutralSession = GameSession<SealedLevel1Backend>;

const MAX_COMMAND_JSON_BYTES: u32 = 4 * 1024;
const MAX_ASSET_ID_BYTES: u32 = 256;
const MAX_EVENT_PAGE_SIZE: u32 = 1024;
const MAX_OUTPUT_JSON_BYTES: u32 = 16 * 1024 * 1024;

fn next_session_id() -> AbiResult<u64> {
    with_registry_mut(Registry::reserve_handle)
}

fn neutral_session_arc(handle: u64) -> AbiResult<Arc<Mutex<NeutralSession>>> {
    if handle == 0 {
        return Err(AbiError::new(
            MC_STATUS_UNKNOWN_HANDLE,
            "neutral session handle zero is invalid",
        ));
    }
    with_registry(|registry| match registry.sessions.get(&handle) {
        Some(super::campaign_abi::RegistryEntry::Neutral(session)) => Ok(Arc::clone(session)),
        _ => Err(AbiError::new(
            MC_STATUS_UNKNOWN_HANDLE,
            format!("unknown neutral session handle {handle}"),
        )),
    })
}

fn with_neutral_session<T>(
    handle: u64,
    operation: impl FnOnce(&NeutralSession) -> AbiResult<T>,
) -> AbiResult<T> {
    let session = neutral_session_arc(handle)?;
    let guard = session
        .lock()
        .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "neutral session lock is poisoned"))?;
    operation(&guard)
}

fn with_neutral_session_mut<T>(
    handle: u64,
    operation: impl FnOnce(&mut NeutralSession) -> AbiResult<T>,
) -> AbiResult<T> {
    let session = neutral_session_arc(handle)?;
    let mut guard = session
        .lock()
        .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "neutral session lock is poisoned"))?;
    operation(&mut guard)
}

fn map_open_error(error: OpenSessionError<AbiError>) -> AbiError {
    match error {
        OpenSessionError::InvalidSessionId => AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "INVALID_SESSION_ID: session ID is zero",
        ),
        OpenSessionError::Adapter(error) => error,
    }
}

fn map_session_error(error: GameSessionError<AbiError>) -> AbiError {
    match error {
        GameSessionError::Backend(error) => error,
        GameSessionError::Guard(error) => {
            let status = match error.code {
                SessionGuardErrorCode::GenerationMismatch
                | SessionGuardErrorCode::GenerationExhausted => MC_STATUS_GENERATION_MISMATCH,
                SessionGuardErrorCode::InvalidSessionId
                | SessionGuardErrorCode::SessionMismatch => MC_STATUS_UNKNOWN_HANDLE,
                SessionGuardErrorCode::TickMismatch => MC_STATUS_INVALID_ARGUMENT,
            };
            AbiError::new(status, error.to_string())
        }
    }
}

fn map_asset_error(error: GameSessionAssetError) -> AbiError {
    match error {
        GameSessionAssetError::Definition(error) => AbiError::new(
            MC_STATUS_VALIDATION_FAILED,
            format!("cannot resolve neutral presentation asset: {error}"),
        ),
        GameSessionAssetError::Guard(error) => {
            let status = match error.code {
                SessionGuardErrorCode::GenerationMismatch
                | SessionGuardErrorCode::GenerationExhausted => MC_STATUS_GENERATION_MISMATCH,
                SessionGuardErrorCode::InvalidSessionId
                | SessionGuardErrorCode::SessionMismatch => MC_STATUS_UNKNOWN_HANDLE,
                SessionGuardErrorCode::TickMismatch => MC_STATUS_INVALID_ARGUMENT,
            };
            AbiError::new(status, error.to_string())
        }
    }
}

fn session_guard(handle: u64, generation: u64, expected_tick: u64) -> SessionGuard {
    SessionGuard {
        session_id: handle,
        generation,
        expected_tick,
    }
}

fn open_neutral_session(inputs: Mc2OwnedInputs, definition: GameDefinition) -> AbiResult<u64> {
    let handle = next_session_id()?;
    let session = GameSession::open(&SealedLevel1Adapter, Arc::new(definition), handle, inputs)
        .map_err(map_open_error)?;
    with_registry_mut(|registry| registry.insert_neutral(handle, session))?;
    Ok(handle)
}

fn explicit_owner_inputs(
    package_dir: *const u8,
    package_dir_len: u32,
    closure_path: *const u8,
    closure_path_len: u32,
    asset_cache_path: *const u8,
    asset_cache_path_len: u32,
) -> AbiResult<Mc2OwnedInputs> {
    let package = utf8_input(package_dir, package_dir_len, "package_dir")?;
    let closure = utf8_input(closure_path, closure_path_len, "closure_path")?;
    let cache = utf8_input(asset_cache_path, asset_cache_path_len, "asset_cache_path")?;
    if package.is_empty() || closure.is_empty() || cache.is_empty() {
        return Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "package_dir, closure_path, and asset_cache_path must be non-empty",
        ));
    }
    Ok(Mc2OwnedInputs::new(package, closure, cache))
}

fn write_json<T: serde::Serialize>(
    value: &T,
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> AbiResult<()> {
    require_output(out_length, "neutral JSON out_length")?;
    let bytes = serde_json::to_vec(value).map_err(|error| {
        AbiError::new(
            MC_STATUS_VALIDATION_FAILED,
            format!("cannot serialize neutral session observation: {error}"),
        )
    })?;
    let length = u32::try_from(bytes.len())
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "neutral JSON exceeds u32"))?;
    if length > MAX_OUTPUT_JSON_BYTES {
        return Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!("neutral JSON requires {length} bytes, maximum is {MAX_OUTPUT_JSON_BYTES}"),
        ));
    }
    write_output(out_length, length);
    if capacity < length {
        return Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!("neutral JSON requires {length} bytes, capacity is {capacity}"),
        ));
    }
    if length > 0 && buffer.is_null() {
        return Err(AbiError::new(
            MC_STATUS_NULL_POINTER,
            "neutral JSON output buffer is null",
        ));
    }
    // SAFETY: the caller supplied a writable buffer with at least `length`
    // bytes, validated above. JSON bytes do not require alignment.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, bytes.len()) };
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_session_contract_json_v1(
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    boundary(|| {
        let contract = serde_json::json!({
            "engine_contract": sim_core::game_session::GAME_SESSION_CONTRACT,
            "transport_contract": "carpet-flight-native-json/v1",
            "transport_version": 1,
            "encoding": "utf-8-json",
            "u64_counter_encoding": "decimal-string",
            "u64_hash_encoding": "lowercase-hex-16",
            "capabilities": ["snapshot", "step", "reset", "events", "audio", "presentation_assets"],
            "limits": {
                "maximum_asset_id_bytes": MAX_ASSET_ID_BYTES,
                "maximum_command_json_bytes": MAX_COMMAND_JSON_BYTES,
                "maximum_event_page_size": MAX_EVENT_PAGE_SIZE,
                "maximum_output_json_bytes": MAX_OUTPUT_JSON_BYTES,
                "json_length_type": "u32"
            },
            "unknown_command_fields": "reject"
        });
        write_json(&contract, buffer, capacity, out_length)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_session_open_v1(
    package_dir: *const u8,
    package_dir_len: u32,
    closure_path: *const u8,
    closure_path_len: u32,
    asset_cache_path: *const u8,
    asset_cache_path_len: u32,
    out_handle: *mut u64,
) -> i32 {
    boundary_value(out_handle, struct_size::<u64>(), || {
        let inputs = explicit_owner_inputs(
            package_dir,
            package_dir_len,
            closure_path,
            closure_path_len,
            asset_cache_path,
            asset_cache_path_len,
        )?;
        let catalog = default_catalog().map_err(|error| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!("cannot construct neutral semantic catalog: {error}"),
            )
        })?;
        let definition = load_mc2_owned(&inputs, &catalog).map_err(|error| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!("cannot load neutral game definition: {error}"),
            )
        })?;
        open_neutral_session(inputs, definition)
    })
}

/// Open the owner-backed session using the active immutable definition in an
/// F3 workspace. The adapter rejects any package or read-only substitution.
#[unsafe(no_mangle)]
pub extern "C" fn mc_game_session_open_workspace_v1(
    package_dir: *const u8,
    package_dir_len: u32,
    closure_path: *const u8,
    closure_path_len: u32,
    asset_cache_path: *const u8,
    asset_cache_path_len: u32,
    workspace_path: *const u8,
    workspace_path_len: u32,
    out_handle: *mut u64,
) -> i32 {
    boundary_value(out_handle, struct_size::<u64>(), || {
        let inputs = explicit_owner_inputs(
            package_dir,
            package_dir_len,
            closure_path,
            closure_path_len,
            asset_cache_path,
            asset_cache_path_len,
        )?;
        let workspace = utf8_input(workspace_path, workspace_path_len, "workspace_path")?;
        if workspace.is_empty() {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "workspace_path must be non-empty",
            ));
        }
        let definition = WorkspaceStore::open(workspace)
            .active_definition()
            .map_err(|error| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    format!("cannot load active workspace definition: {error}"),
                )
            })?;
        open_neutral_session(inputs, definition)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_session_destroy_v1(handle: u64) -> i32 {
    boundary(|| with_registry_mut(|registry| registry.remove_neutral(handle)))
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_session_snapshot_json_v1(
    handle: u64,
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    boundary(|| {
        with_neutral_session(handle, |session| {
            write_json(&session.snapshot(), buffer, capacity, out_length)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_session_step_json_v1(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    command_json: *const u8,
    command_json_len: u32,
) -> i32 {
    boundary(|| {
        let command_json = utf8_input(command_json, command_json_len, "neutral command JSON")?;
        if command_json_len > MAX_COMMAND_JSON_BYTES {
            return Err(AbiError::new(
                MC_STATUS_BOUNDS,
                format!(
                    "neutral command JSON is {command_json_len} bytes, maximum is {MAX_COMMAND_JSON_BYTES}"
                ),
            ));
        }
        let command: GameCommand =
            serde_json::from_slice(command_json.as_bytes()).map_err(|error| {
                AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("invalid neutral command JSON: {error}"),
                )
            })?;
        with_neutral_session_mut(handle, |session| {
            session
                .step(session_guard(handle, generation, expected_tick), command)
                .map(|_| ())
                .map_err(map_session_error)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_session_reset_v1(
    handle: u64,
    generation: u64,
    expected_tick: u64,
) -> i32 {
    boundary(|| {
        with_neutral_session_mut(handle, |session| {
            session
                .reset(session_guard(handle, generation, expected_tick))
                .map(|_| ())
                .map_err(map_session_error)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_session_events_json_v1(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    cursor: u64,
    limit: u32,
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    boundary(|| {
        if limit > MAX_EVENT_PAGE_SIZE {
            return Err(AbiError::new(
                MC_STATUS_BOUNDS,
                format!("event limit {limit} exceeds maximum {MAX_EVENT_PAGE_SIZE}"),
            ));
        }
        with_neutral_session(handle, |session| {
            let page = session
                .events_after(
                    session_guard(handle, generation, expected_tick),
                    cursor,
                    usize::try_from(limit).map_err(|_| {
                        AbiError::new(MC_STATUS_BOUNDS, "event limit exceeds usize")
                    })?,
                )
                .map_err(map_session_error)?;
            write_json(&page, buffer, capacity, out_length)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_session_audio_json_v1(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    event_sequence: u64,
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    boundary(|| {
        with_neutral_session(handle, |session| {
            let command = session
                .audio_for_event(
                    session_guard(handle, generation, expected_tick),
                    event_sequence,
                )
                .map_err(map_session_error)?;
            write_json(&command, buffer, capacity, out_length)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_session_presentation_asset_json_v1(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    semantic_id: *const u8,
    semantic_id_len: u32,
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    boundary(|| {
        if semantic_id_len == 0 || semantic_id_len > MAX_ASSET_ID_BYTES {
            return Err(AbiError::new(
                MC_STATUS_BOUNDS,
                format!("semantic asset ID length must be 1..={MAX_ASSET_ID_BYTES}"),
            ));
        }
        let semantic_id = utf8_input(semantic_id, semantic_id_len, "semantic asset ID")?;
        let semantic_id = sim_core::game_definition::SemanticId::parse(&semantic_id)
            .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error.to_string()))?;
        with_neutral_session(handle, |session| {
            let asset = session
                .presentation_asset(
                    session_guard(handle, generation, expected_tick),
                    &semantic_id,
                )
                .map_err(map_asset_error)?;
            write_json(&asset, buffer, capacity, out_length)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::game_definition::{
        ChangeSetBuilder, PublicationFault, RequestedTiming, SemanticId, SemanticValue,
    };

    fn fixture_paths() -> (PathBuf, PathBuf, PathBuf) {
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        (
            private.join("generated/level1-export-m"),
            private.join("generated/level1-asset-closure.json"),
            private.join("generated/level1-unity-asset-cache-plan29-final"),
        )
    }

    fn open_fixture() -> u64 {
        let (package, closure, cache) = fixture_paths();
        let package = package.to_string_lossy();
        let closure = closure.to_string_lossy();
        let cache = cache.to_string_lossy();
        let mut handle = 0;
        assert_eq!(
            mc_game_session_open_v1(
                package.as_ptr(),
                package.len() as u32,
                closure.as_ptr(),
                closure.len() as u32,
                cache.as_ptr(),
                cache.len() as u32,
                &raw mut handle,
            ),
            MC_STATUS_OK
        );
        handle
    }

    fn read_json(operation: impl FnOnce(*mut u8, u32, *mut u32) -> i32) -> serde_json::Value {
        let mut bytes = vec![0_u8; 16 * 1024];
        let mut length = 0;
        assert_eq!(
            operation(bytes.as_mut_ptr(), bytes.len() as u32, &raw mut length),
            MC_STATUS_OK
        );
        serde_json::from_slice(&bytes[..length as usize]).unwrap()
    }

    #[test]
    fn external_neutral_abi_opens_steps_pages_and_rejects_stale_or_unknown_commands() {
        let contract = read_json(|buffer, capacity, length| {
            mc_game_session_contract_json_v1(buffer, capacity, length)
        });
        assert_eq!(contract["engine_contract"], "carpet-flight-game-session/v1");
        assert_eq!(
            contract["transport_contract"],
            "carpet-flight-native-json/v1"
        );
        assert_eq!(contract["transport_version"], 1);
        assert_eq!(contract["u64_counter_encoding"], "decimal-string");
        assert_eq!(contract["u64_hash_encoding"], "lowercase-hex-16");
        assert_eq!(contract["limits"]["maximum_command_json_bytes"], 4_096);
        assert_eq!(contract["limits"]["maximum_asset_id_bytes"], 256);
        assert_eq!(contract["limits"]["maximum_event_page_size"], 1_024);
        assert_eq!(contract["limits"]["maximum_output_json_bytes"], 16_777_216);
        assert_eq!(contract["unknown_command_fields"], "reject");
        let handle = open_fixture();
        let initial = read_json(|buffer, capacity, length| {
            mc_game_session_snapshot_json_v1(handle, buffer, capacity, length)
        });
        assert_eq!(initial["identity"]["session_id"], handle.to_string());
        assert_eq!(initial["identity"]["generation"], "1");
        assert_eq!(initial["identity"]["tick"], "0");

        let unknown = br#"{"source_opcode":1}"#;
        assert_eq!(
            mc_game_session_step_json_v1(handle, 1, 0, unknown.as_ptr(), unknown.len() as u32),
            MC_STATUS_INVALID_ARGUMENT
        );
        let unchanged = read_json(|buffer, capacity, length| {
            mc_game_session_snapshot_json_v1(handle, buffer, capacity, length)
        });
        assert_eq!(unchanged, initial);

        let command = br#"{"longitudinal":"accelerate","roll_axis":38}"#;
        assert_eq!(
            mc_game_session_step_json_v1(handle, 1, 0, command.as_ptr(), command.len() as u32),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_game_session_step_json_v1(handle, 1, 0, command.as_ptr(), command.len() as u32),
            MC_STATUS_INVALID_ARGUMENT
        );
        let stepped = read_json(|buffer, capacity, length| {
            mc_game_session_snapshot_json_v1(handle, buffer, capacity, length)
        });
        assert_eq!(stepped["identity"]["tick"], "1");
        let mut compatibility = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_snapshot(
                handle,
                &raw mut compatibility,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(compatibility.generation, 1);
        assert_eq!(compatibility.tick, 1);
        assert_eq!(
            format!("{:016x}", compatibility.state_hash),
            stepped["value"]["state_hash"]
        );
        assert_eq!(
            format!("{:016x}", compatibility.conformance_hash),
            stepped["value"]["conformance_hash"]
        );
        let before_oversized_page = stepped.clone();
        let mut oversized_page_length = 0;
        assert_eq!(
            mc_game_session_events_json_v1(
                handle,
                1,
                1,
                0,
                1_025,
                std::ptr::null_mut(),
                0,
                &raw mut oversized_page_length,
            ),
            MC_STATUS_BOUNDS
        );
        let after_oversized_page = read_json(|buffer, capacity, length| {
            mc_game_session_snapshot_json_v1(handle, buffer, capacity, length)
        });
        assert_eq!(after_oversized_page, before_oversized_page);
        let events = read_json(|buffer, capacity, length| {
            mc_game_session_events_json_v1(handle, 1, 1, 0, 1_024, buffer, capacity, length)
        });
        assert!(
            events["value"]["events"]
                .as_array()
                .is_some_and(|rows| !rows.is_empty())
        );
        assert!(!events.to_string().contains("level1"));
        assert!(!events.to_string().contains("source_slot"));
        assert!(!events.to_string().contains("source_opcode"));
        assert!(
            events["value"]["events"]
                .as_array()
                .unwrap()
                .iter()
                .all(|event| event.get("variant").is_none() && event.get("values").is_none())
        );

        let before_short = stepped;
        let mut required = 0;
        assert_eq!(
            mc_game_session_snapshot_json_v1(handle, std::ptr::null_mut(), 0, &raw mut required),
            MC_STATUS_BOUNDS
        );
        assert!(required > 0);
        let after_short = read_json(|buffer, capacity, length| {
            mc_game_session_snapshot_json_v1(handle, buffer, capacity, length)
        });
        assert_eq!(after_short, before_short);

        assert_eq!(mc_game_session_reset_v1(handle, 1, 1), MC_STATUS_OK);
        let reset = read_json(|buffer, capacity, length| {
            mc_game_session_snapshot_json_v1(handle, buffer, capacity, length)
        });
        assert_eq!(reset["identity"]["generation"], "2");
        assert_eq!(reset["identity"]["tick"], "0");
        let mut stale_audio_length = 0;
        assert_eq!(
            mc_game_session_audio_json_v1(
                handle,
                1,
                0,
                1,
                std::ptr::null_mut(),
                0,
                &raw mut stale_audio_length,
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(stale_audio_length, 0);
        let after_stale_audio = read_json(|buffer, capacity, length| {
            mc_game_session_snapshot_json_v1(handle, buffer, capacity, length)
        });
        assert_eq!(after_stale_audio, reset);
        assert_eq!(mc_game_session_destroy_v1(handle), MC_STATUS_OK);
    }

    #[test]
    fn portable_asset_binding_is_observational_and_follows_forward_publication_and_rollback() {
        use sim_core::game_definition::{GamePackage, engine_capabilities};

        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../sim-core/tests/fixtures/custom-synthetic-v1");
        let catalog = default_catalog().unwrap();
        let original = GameDefinition::from_package(
            GamePackage::load(fixture).unwrap(),
            &engine_capabilities().unwrap(),
            &catalog,
        )
        .unwrap();
        let workspace = std::env::temp_dir().join(format!(
            "carpet-flight-f9-native-workspace-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&workspace);
        let store = WorkspaceStore::create(&workspace, "f9-native", original.clone()).unwrap();
        let change_set = ChangeSetBuilder::new(
            "f9-native",
            &original,
            "f9-variant",
            RequestedTiming::PackageReload,
        )
        .replace(
            SemanticId::parse("presentation.spell.fireball.projectile_asset").unwrap(),
            SemanticValue::Reference(
                "sha256:c924ac200c84b9983374d1abcacff27414c3f565ac82792394353e0358ee0b34"
                    .to_owned(),
            ),
        )
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted);
        let applied = store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();

        let (package, closure, cache) = fixture_paths();
        let package = package.to_string_lossy();
        let closure = closure.to_string_lossy();
        let cache = cache.to_string_lossy();
        let workspace_text = workspace.to_string_lossy();
        let open = || {
            let mut handle = 0;
            assert_eq!(
                mc_game_session_open_workspace_v1(
                    package.as_ptr(),
                    package.len() as u32,
                    closure.as_ptr(),
                    closure.len() as u32,
                    cache.as_ptr(),
                    cache.len() as u32,
                    workspace_text.as_ptr(),
                    workspace_text.len() as u32,
                    &raw mut handle,
                ),
                MC_STATUS_OK
            );
            handle
        };
        let slot = b"presentation.spell.fireball.projectile_asset";
        let variant_handle = open();
        let before = read_json(|buffer, capacity, length| {
            mc_game_session_snapshot_json_v1(variant_handle, buffer, capacity, length)
        });
        let variant_asset = read_json(|buffer, capacity, length| {
            mc_game_session_presentation_asset_json_v1(
                variant_handle,
                1,
                0,
                slot.as_ptr(),
                slot.len() as u32,
                buffer,
                capacity,
                length,
            )
        });
        let after = read_json(|buffer, capacity, length| {
            mc_game_session_snapshot_json_v1(variant_handle, buffer, capacity, length)
        });
        assert_eq!(before, after);
        assert_eq!(
            variant_asset["value"]["content_address"],
            "sha256:c924ac200c84b9983374d1abcacff27414c3f565ac82792394353e0358ee0b34"
        );
        assert_eq!(variant_asset["value"]["width"], 3);
        assert_eq!(variant_asset["value"]["height"], 3);
        assert_eq!(
            variant_asset["value"]["indices"].as_array().unwrap().len(),
            9
        );
        assert_eq!(mc_game_session_destroy_v1(variant_handle), MC_STATUS_OK);

        let active = store.active_definition().unwrap();
        let rollback = store
            .rollback(
                active.definition_hash(),
                active.revision(),
                &applied.transaction_id,
                "f9-rollback",
                PublicationFault::None,
            )
            .unwrap();
        assert_eq!(rollback.resulting_definition_revision, 3);
        let rollback_handle = open();
        let rolled_asset = read_json(|buffer, capacity, length| {
            mc_game_session_presentation_asset_json_v1(
                rollback_handle,
                1,
                0,
                slot.as_ptr(),
                slot.len() as u32,
                buffer,
                capacity,
                length,
            )
        });
        assert_eq!(
            rolled_asset["value"]["content_address"],
            "sha256:11200749e51e49d4a9f4be95ffa8f6c0636be87e36781481c89e6a0825232508"
        );
        assert_eq!(mc_game_session_destroy_v1(rollback_handle), MC_STATUS_OK);
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn workspace_open_consumes_the_published_flight_value_in_the_same_native_step() {
        let (package, closure, cache) = fixture_paths();
        let inputs = Mc2OwnedInputs::new(&package, &closure, &cache);
        let catalog = default_catalog().unwrap();
        let original = load_mc2_owned(&inputs, &catalog).unwrap();
        let workspace = std::env::temp_dir().join(format!(
            "carpet-flight-f6-native-workspace-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&workspace);
        let store = WorkspaceStore::create(&workspace, "f6-native", original).unwrap();
        let active = store.active_definition().unwrap();
        let change_set = ChangeSetBuilder::new(
            "f6-native",
            &active,
            "flight-limit-16",
            RequestedTiming::SessionReset,
        )
        .replace(
            SemanticId::parse("flight.player.max_forward_speed").unwrap(),
            SemanticValue::Integer(16),
        )
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();

        let package = package.to_string_lossy();
        let closure = closure.to_string_lossy();
        let cache = cache.to_string_lossy();
        let workspace_text = workspace.to_string_lossy();
        let mut handle = 0;
        assert_eq!(
            mc_game_session_open_workspace_v1(
                package.as_ptr(),
                package.len() as u32,
                closure.as_ptr(),
                closure.len() as u32,
                cache.as_ptr(),
                cache.len() as u32,
                workspace_text.as_ptr(),
                workspace_text.len() as u32,
                &raw mut handle,
            ),
            MC_STATUS_OK
        );
        let command = br#"{"longitudinal":"accelerate"}"#;
        for tick in 0..2 {
            assert_eq!(
                mc_game_session_step_json_v1(
                    handle,
                    1,
                    tick,
                    command.as_ptr(),
                    command.len() as u32,
                ),
                MC_STATUS_OK
            );
        }
        let snapshot = read_json(|buffer, capacity, length| {
            mc_game_session_snapshot_json_v1(handle, buffer, capacity, length)
        });
        assert_eq!(snapshot["value"]["player_motion"]["target_speed"], 16);
        assert_eq!(snapshot["identity"]["definition_revision"], "2");
        assert_eq!(mc_game_session_destroy_v1(handle), MC_STATUS_OK);
        let _ = std::fs::remove_dir_all(workspace);
    }
}
