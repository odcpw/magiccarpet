//! Rust-owned title/loading/playing runtime transport.
//!
//! A runtime remains behind one handle for its entire lifetime. Once it
//! reaches `playing`, the existing `mc_game_session_*_v1` exports operate on
//! the nested `GameSession`; this module does not create a second gameplay
//! authority or a parallel step API.

use super::*;
use serde::Deserialize;
use sim_core::{
    campaign::backend::{
        MAX_MC2_CAMPAIGN_REPLAY_SAVE_BYTES, MC2_CAMPAIGN_REPLAY_SAVE_CONTRACT, Mc2CampaignBackend,
        Mc2CampaignInputs, Mc2CampaignReplaySaveError, Mc2RuntimePackageResolver,
    },
    game_runtime::{
        GameRuntime, OpenLevelError, PausedFrameError, RuntimeGuard, RuntimeGuardError,
        RuntimeGuardErrorCode, RuntimeMode, RuntimeTransitionError,
    },
};
use sim_session::{GameSession, OpenSessionError, SessionGuard};

const GAME_RUNTIME_TRANSPORT_CONTRACT: &str = "carpet-flight-native-game-runtime-json/v8";
const INSTALLED_PACKAGE_SET_CONTRACT: &str = "carpet-flight-installed-package-set-bootstrap/v1";
const MAX_INSTALLED_PACKAGE_SET_JSON_BYTES: u32 = 64 * 1024;
const MAX_INSTALLED_PACKAGE_COUNT: usize = 16;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstalledPackageSetRequest {
    request_contract: String,
    packages: Vec<InstalledPackageLocation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstalledPackageLocation {
    package_dir: String,
    owner_cache_path: Option<String>,
}

pub(super) struct RuntimeRecord {
    runtime: GameRuntime<Mc2CampaignBackend>,
    resolver: Option<Mc2RuntimePackageResolver>,
    closed: bool,
}

impl RuntimeRecord {
    fn title(runtime_id: u64) -> AbiResult<Self> {
        let runtime = GameRuntime::title(runtime_id).map_err(map_transition_error)?;
        Ok(Self {
            runtime,
            resolver: None,
            closed: false,
        })
    }

    fn ensure_open(&self) -> AbiResult<()> {
        if self.closed {
            return Err(AbiError::new(
                MC_STATUS_UNKNOWN_HANDLE,
                "game runtime is closed",
            ));
        }
        Ok(())
    }

    pub(super) fn session(&self) -> AbiResult<&GameSession<Mc2CampaignBackend>> {
        self.ensure_open()?;
        self.runtime.session().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                format!(
                    "INVALID_RUNTIME_MODE: gameplay session is unavailable while runtime is {}",
                    self.runtime.identity().mode.as_str()
                ),
            )
        })
    }

    pub(super) fn session_mut(&mut self) -> AbiResult<&mut GameSession<Mc2CampaignBackend>> {
        self.ensure_open()?;
        let mode = self.runtime.identity().mode;
        self.runtime.session_mut().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                format!(
                    "INVALID_RUNTIME_MODE: gameplay session is unavailable while runtime is {}",
                    mode.as_str()
                ),
            )
        })
    }

    fn bootstrap_package_set(
        &mut self,
        handle: u64,
        generation: u64,
        request_json: String,
        request_json_len: u32,
    ) -> AbiResult<()> {
        self.validate_transition(handle, generation, RuntimeMode::Title)?;
        if self.resolver.is_some() {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "PACKAGE_SET_ALREADY_INSTALLED: runtime package authority is immutable until destroy",
            ));
        }
        let inputs = installed_package_set_inputs(request_json, request_json_len)?;
        let resolver = Mc2RuntimePackageResolver::load(inputs).map_err(|error| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!("cannot load installed game runtime package set: {error}"),
            )
        })?;
        resolver
            .new_game(
                &mut self.runtime,
                RuntimeGuard {
                    runtime_id: handle,
                    generation,
                },
            )
            .map_err(map_transition_error)?;
        self.resolver = Some(resolver);
        Ok(())
    }

    fn open_level(&mut self, handle: u64, generation: u64) -> AbiResult<()> {
        self.validate_transition(handle, generation, RuntimeMode::Loading)?;
        let guard = RuntimeGuard {
            runtime_id: handle,
            generation,
        };
        let resolver = self.resolver.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "MISSING_RUNTIME_RESOLVER: loading runtime has no installed package set",
            )
        })?;
        resolver
            .open_pending(&mut self.runtime, guard, handle)
            .map_err(map_open_level_error)?;
        Ok(())
    }

    fn restart_new_game(&mut self, handle: u64, generation: u64) -> AbiResult<()> {
        self.validate_transition(handle, generation, RuntimeMode::Title)?;
        self.resolver
            .as_ref()
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "MISSING_RUNTIME_RESOLVER: restart requires an installed package set",
                )
            })?
            .new_game(
                &mut self.runtime,
                RuntimeGuard {
                    runtime_id: handle,
                    generation,
                },
            )
            .map_err(map_transition_error)?;
        Ok(())
    }

    fn save_replay(
        &self,
        handle: u64,
        runtime_generation: u64,
        session_generation: u64,
        expected_tick: u64,
    ) -> AbiResult<Vec<u8>> {
        let mode = self.runtime.identity().mode;
        if !matches!(mode, RuntimeMode::Playing | RuntimeMode::Paused) {
            return Err(map_transition_error(RuntimeTransitionError::InvalidMode {
                expected: RuntimeMode::Playing,
                actual: mode,
            }));
        }
        self.validate_transition(handle, runtime_generation, mode)?;
        let session = self.runtime.session().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "INVALID_RUNTIME_MODE: Playing runtime has no gameplay session",
            )
        })?;
        let identity = session.identity();
        if identity.generation != session_generation {
            return Err(AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                format!(
                    "GENERATION_MISMATCH: expected session generation {}, got {session_generation}",
                    identity.generation
                ),
            ));
        }
        if identity.tick != expected_tick {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                format!(
                    "TICK_MISMATCH: expected session tick {}, got {expected_tick}",
                    identity.tick
                ),
            ));
        }
        self.resolver
            .as_ref()
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "MISSING_RUNTIME_RESOLVER: Playing runtime has no installed package set",
                )
            })?
            .save_replay(&self.runtime)
            .map_err(map_replay_save_error)
    }

    fn load_replay(
        &mut self,
        handle: u64,
        generation: u64,
        package_set_json: String,
        package_set_json_len: u32,
        save_json: &[u8],
    ) -> AbiResult<()> {
        self.validate_transition(handle, generation, RuntimeMode::Title)?;
        if self.resolver.is_some() {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "PACKAGE_SET_ALREADY_INSTALLED: replay load requires a fresh runtime",
            ));
        }
        let inputs = installed_package_set_inputs(package_set_json, package_set_json_len)?;
        let resolver = Mc2RuntimePackageResolver::load(inputs).map_err(|error| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!("cannot load replay package set: {error}"),
            )
        })?;
        resolver
            .load_replay(
                &mut self.runtime,
                RuntimeGuard {
                    runtime_id: handle,
                    generation,
                },
                handle,
                save_json,
            )
            .map_err(map_replay_save_error)?;
        self.resolver = Some(resolver);
        Ok(())
    }

    fn pause(&mut self, handle: u64, generation: u64) -> AbiResult<()> {
        self.validate_transition(handle, generation, RuntimeMode::Playing)?;
        self.runtime
            .pause(RuntimeGuard {
                runtime_id: handle,
                generation,
            })
            .map_err(map_transition_error)?;
        Ok(())
    }

    fn paused_frame(
        &mut self,
        handle: u64,
        runtime_generation: u64,
        session_generation: u64,
        expected_tick: u64,
    ) -> AbiResult<()> {
        self.validate_transition(handle, runtime_generation, RuntimeMode::Paused)?;
        self.runtime
            .paused_frame(
                RuntimeGuard {
                    runtime_id: handle,
                    generation: runtime_generation,
                },
                SessionGuard {
                    session_id: handle,
                    generation: session_generation,
                    expected_tick,
                },
            )
            .map_err(map_paused_frame_error)?;
        Ok(())
    }

    fn resume(&mut self, handle: u64, generation: u64) -> AbiResult<()> {
        self.validate_transition(handle, generation, RuntimeMode::Paused)?;
        self.runtime
            .resume(RuntimeGuard {
                runtime_id: handle,
                generation,
            })
            .map_err(map_transition_error)?;
        Ok(())
    }

    fn close(
        &mut self,
        handle: u64,
        runtime_generation: u64,
        session_generation: u64,
        expected_tick: u64,
    ) -> AbiResult<()> {
        self.runtime
            .close(
                RuntimeGuard {
                    runtime_id: handle,
                    generation: runtime_generation,
                },
                SessionGuard {
                    session_id: handle,
                    generation: session_generation,
                    expected_tick,
                },
            )
            .map_err(map_transition_error)?;
        Ok(())
    }

    fn validate_transition(
        &self,
        handle: u64,
        generation: u64,
        expected_mode: RuntimeMode,
    ) -> AbiResult<()> {
        self.ensure_open()?;
        let expected = self.runtime.identity();
        let actual = RuntimeGuard {
            runtime_id: handle,
            generation,
        };
        let guard_code = if handle != expected.runtime_id {
            Some(RuntimeGuardErrorCode::RuntimeMismatch)
        } else if generation != expected.generation {
            Some(RuntimeGuardErrorCode::GenerationMismatch)
        } else {
            None
        };
        if let Some(code) = guard_code {
            return Err(map_transition_error(RuntimeTransitionError::Guard(
                Box::new(RuntimeGuardError {
                    code,
                    expected,
                    actual,
                }),
            )));
        }
        if expected.mode != expected_mode {
            return Err(map_transition_error(RuntimeTransitionError::InvalidMode {
                expected: expected_mode,
                actual: expected.mode,
            }));
        }
        Ok(())
    }
}

fn runtime_arc(handle: u64) -> AbiResult<Arc<Mutex<super::game_session_abi::NeutralSession>>> {
    if handle == 0 {
        return Err(AbiError::new(
            MC_STATUS_UNKNOWN_HANDLE,
            "game runtime handle zero is invalid",
        ));
    }
    with_registry(|registry| match registry.sessions.get(&handle) {
        Some(super::campaign_abi::RegistryEntry::Neutral(runtime)) => Ok(Arc::clone(runtime)),
        _ => Err(AbiError::new(
            MC_STATUS_UNKNOWN_HANDLE,
            format!("unknown game runtime handle {handle}"),
        )),
    })
}

fn with_runtime<T>(
    handle: u64,
    operation: impl FnOnce(&RuntimeRecord) -> AbiResult<T>,
) -> AbiResult<T> {
    let entry = runtime_arc(handle)?;
    let guard = entry
        .lock()
        .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "game runtime lock is poisoned"))?;
    match &*guard {
        super::game_session_abi::NeutralSession::Runtime(runtime) => {
            runtime.ensure_open()?;
            operation(runtime)
        }
        _ => Err(AbiError::new(
            MC_STATUS_UNKNOWN_HANDLE,
            format!("handle {handle} is not a game runtime"),
        )),
    }
}

fn with_runtime_mut<T>(
    handle: u64,
    operation: impl FnOnce(&mut RuntimeRecord) -> AbiResult<T>,
) -> AbiResult<T> {
    let entry = runtime_arc(handle)?;
    let mut guard = entry
        .lock()
        .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "game runtime lock is poisoned"))?;
    match &mut *guard {
        super::game_session_abi::NeutralSession::Runtime(runtime) => {
            runtime.ensure_open()?;
            operation(runtime)
        }
        _ => Err(AbiError::new(
            MC_STATUS_UNKNOWN_HANDLE,
            format!("handle {handle} is not a game runtime"),
        )),
    }
}

fn map_transition_error(error: RuntimeTransitionError) -> AbiError {
    if let RuntimeTransitionError::SessionGuard(error) = error {
        return super::game_session_abi::map_guard_error(*error);
    }
    let status = match &error {
        RuntimeTransitionError::Guard(error)
            if matches!(error.code, RuntimeGuardErrorCode::GenerationMismatch) =>
        {
            MC_STATUS_GENERATION_MISMATCH
        }
        RuntimeTransitionError::InvalidRuntimeId | RuntimeTransitionError::Guard(_) => {
            MC_STATUS_UNKNOWN_HANDLE
        }
        RuntimeTransitionError::SessionGuard(_) => {
            unreachable!("session guard errors return before status matching")
        }
        RuntimeTransitionError::InvalidMode { .. } => MC_STATUS_INVALID_ARGUMENT,
        RuntimeTransitionError::InvalidRuntimeFlow { .. }
        | RuntimeTransitionError::InvalidNextDefinition { .. }
        | RuntimeTransitionError::InvalidReplay { .. }
        | RuntimeTransitionError::InvalidCompletion { .. } => MC_STATUS_VALIDATION_FAILED,
        RuntimeTransitionError::CompletionNotReady => MC_STATUS_INVALID_ARGUMENT,
        RuntimeTransitionError::GenerationExhausted => MC_STATUS_GENERATION_MISMATCH,
    };
    AbiError::new(status, error.to_string())
}

fn map_replay_save_error(error: Mc2CampaignReplaySaveError) -> AbiError {
    match error {
        Mc2CampaignReplaySaveError::InvalidMode => AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "replay save requires Playing mode",
        ),
        Mc2CampaignReplaySaveError::InvalidSave { message } => AbiError::new(
            MC_STATUS_VALIDATION_FAILED,
            format!("INVALID_REPLAY_SAVE: {message}"),
        ),
        Mc2CampaignReplaySaveError::Definition(error) => AbiError::new(
            MC_STATUS_VALIDATION_FAILED,
            format!("INVALID_REPLAY_DEFINITION: {error}"),
        ),
        Mc2CampaignReplaySaveError::Runtime(error) => map_transition_error(error),
        Mc2CampaignReplaySaveError::Restore(
            sim_core::game_runtime::RestorePlayingError::Transition(error),
        ) => map_transition_error(error),
        Mc2CampaignReplaySaveError::Restore(
            sim_core::game_runtime::RestorePlayingError::Session(
                OpenSessionError::InvalidSessionId,
            ),
        ) => AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "INVALID_SESSION_ID: replay target session ID is zero",
        ),
        Mc2CampaignReplaySaveError::Restore(
            sim_core::game_runtime::RestorePlayingError::Session(OpenSessionError::Adapter(error)),
        ) => super::game_session_abi::map_campaign_error(error),
        Mc2CampaignReplaySaveError::Restore(
            sim_core::game_runtime::RestorePlayingError::ReplayStep {
                command_index,
                error,
            },
        ) => AbiError::new(
            MC_STATUS_VALIDATION_FAILED,
            format!("REPLAY_STEP_REJECTED at command {command_index}: {error}"),
        ),
        Mc2CampaignReplaySaveError::Restore(
            sim_core::game_runtime::RestorePlayingError::ReplayPausedFrame {
                operation_index,
                error,
            },
        ) => AbiError::new(
            MC_STATUS_VALIDATION_FAILED,
            format!("REPLAY_PAUSED_FRAME_REJECTED at operation {operation_index}: {error}"),
        ),
    }
}

fn map_paused_frame_error(
    error: PausedFrameError<sim_core::campaign::backend::CampaignBackendError>,
) -> AbiError {
    match error {
        PausedFrameError::Transition(error) => map_transition_error(error),
        PausedFrameError::Session(sim_session::GameSessionError::Guard(error)) => {
            super::game_session_abi::map_guard_error(error)
        }
        PausedFrameError::Session(sim_session::GameSessionError::Backend(error)) => {
            super::game_session_abi::map_campaign_error(error)
        }
    }
}

fn map_open_level_error(
    error: OpenLevelError<sim_core::campaign::backend::CampaignBackendError>,
) -> AbiError {
    match error {
        OpenLevelError::Transition(error) => map_transition_error(error),
        OpenLevelError::Session(OpenSessionError::InvalidSessionId) => AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "INVALID_SESSION_ID: nested runtime session ID is zero",
        ),
        OpenLevelError::Session(OpenSessionError::Adapter(error)) => {
            super::game_session_abi::map_campaign_error(error)
        }
    }
}

fn campaign_inputs(package: String, owner_cache: String) -> AbiResult<Mc2CampaignInputs> {
    if package.is_empty() {
        return Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "package_dir must be non-empty",
        ));
    }
    Ok(if owner_cache.is_empty() {
        Mc2CampaignInputs::new(package)
    } else {
        Mc2CampaignInputs::with_owner_cache(package, owner_cache)
    })
}

fn installed_package_set_inputs(
    request_json: String,
    request_json_len: u32,
) -> AbiResult<Vec<Mc2CampaignInputs>> {
    if request_json_len > MAX_INSTALLED_PACKAGE_SET_JSON_BYTES {
        return Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!(
                "installed package-set JSON is {request_json_len} bytes, maximum is {MAX_INSTALLED_PACKAGE_SET_JSON_BYTES}"
            ),
        ));
    }
    let request: InstalledPackageSetRequest =
        serde_json::from_str(&request_json).map_err(|error| {
            AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                format!("invalid installed package-set JSON: {error}"),
            )
        })?;
    if request.request_contract != INSTALLED_PACKAGE_SET_CONTRACT {
        return Err(AbiError::new(
            MC_STATUS_UNSUPPORTED_VERSION,
            format!(
                "unsupported installed package-set contract {}",
                request.request_contract
            ),
        ));
    }
    if request.packages.is_empty() {
        return Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "installed package set must contain at least one resource location",
        ));
    }
    if request.packages.len() > MAX_INSTALLED_PACKAGE_COUNT {
        return Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!(
                "installed package set must contain 1..={MAX_INSTALLED_PACKAGE_COUNT} resource locations"
            ),
        ));
    }
    request
        .packages
        .into_iter()
        .map(|location| {
            if location.package_dir.trim().is_empty()
                || location
                    .owner_cache_path
                    .as_deref()
                    .is_some_and(|path| !path.is_empty() && path.trim().is_empty())
            {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "installed package resource paths must be nonblank",
                ));
            }
            campaign_inputs(
                location.package_dir,
                location.owner_cache_path.unwrap_or_default(),
            )
        })
        .collect()
}

fn write_replay_save(
    bytes: &[u8],
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> AbiResult<()> {
    require_output(out_length, "replay save out_length")?;
    let length = u32::try_from(bytes.len())
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "replay save exceeds u32"))?;
    write_output(out_length, length);
    if capacity < length {
        return Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!("replay save requires {length} bytes, capacity is {capacity}"),
        ));
    }
    if length > 0 && buffer.is_null() {
        return Err(AbiError::new(
            MC_STATUS_NULL_POINTER,
            "replay save output buffer is null",
        ));
    }
    // SAFETY: the caller supplied a writable byte buffer with at least
    // `length` bytes, validated above.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, bytes.len()) };
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_contract_json_v1(
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    boundary(|| {
        let contract = serde_json::json!({
            "runtime_contract": sim_core::game_runtime::GAME_RUNTIME_CONTRACT,
            "transport_contract": GAME_RUNTIME_TRANSPORT_CONTRACT,
            "transport_version": 8,
            "encoding": "utf-8-json",
            "u64_counter_encoding": "decimal-string",
            "modes": ["title", "loading", "playing", "paused"],
            "transitions": ["bootstrap_package_set", "restart_new_game", "open_level", "load_replay_save", "pause", "paused_frame", "resume", "close"],
            "playing_session_transport": "mc_game_session_*_v1 on the same handle",
            "package_profile": "mc2-campaign",
            "installed_package_set": {
                "request_contract": INSTALLED_PACKAGE_SET_CONTRACT,
                "transport": "mc_game_runtime_bootstrap_package_set_json_v1",
                "selection_authority": "rust_owned_semantic_flow",
                "input_order": "unordered_resource_locations",
                "lifetime": "immutable_until_destroy"
            },
            "replay_save": {
                "save_contract": MC2_CAMPAIGN_REPLAY_SAVE_CONTRACT,
                "save_transport": "mc_game_runtime_save_replay_json_v1",
                "load_transport": "mc_game_runtime_load_replay_json_v1",
                "load_package_authority": "complete_unordered_resource_set",
                "client_semantics": "opaque_utf8_json"
            },
            "paused_frame": {
                "transport": "mc_game_runtime_paused_frame_v1",
                "gameplay_tick": "frozen",
                "session_identity": "unchanged",
                "deterministic_work": "package_declared_pre_gate",
                "replay": "ordered_operation"
            },
            "runtime_flow": {
                "title": "absent",
                "loading": "pending_definition.runtime_flow",
                "playing_and_paused": "active_runtime_flow"
            },
            "limits": {
                "maximum_output_json_bytes": super::game_session_abi::MAX_OUTPUT_JSON_BYTES,
                "maximum_installed_package_set_json_bytes": MAX_INSTALLED_PACKAGE_SET_JSON_BYTES,
                "maximum_installed_package_count": MAX_INSTALLED_PACKAGE_COUNT,
                "maximum_replay_save_json_bytes": MAX_MC2_CAMPAIGN_REPLAY_SAVE_BYTES,
                "json_length_type": "u32"
            }
        });
        super::game_session_abi::write_json(&contract, buffer, capacity, out_length)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_create_v1(out_handle: *mut u64) -> i32 {
    boundary_value(out_handle, struct_size::<u64>(), || {
        let handle = with_registry_mut(Registry::reserve_handle)?;
        let runtime = RuntimeRecord::title(handle)?;
        with_registry_mut(|registry| {
            registry.insert_neutral(
                handle,
                super::game_session_abi::NeutralSession::Runtime(runtime),
            )
        })?;
        Ok(handle)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_identity_json_v1(
    handle: u64,
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    boundary(|| {
        with_runtime(handle, |runtime| {
            super::game_session_abi::write_json(
                &runtime.runtime.identity(),
                buffer,
                capacity,
                out_length,
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_bootstrap_package_set_json_v1(
    handle: u64,
    generation: u64,
    request_json: *const u8,
    request_json_len: u32,
) -> i32 {
    boundary(|| {
        if request_json.is_null() && request_json_len != 0 {
            return Err(AbiError::new(
                MC_STATUS_NULL_POINTER,
                "installed package-set JSON pointer is null",
            ));
        }
        if request_json_len > MAX_INSTALLED_PACKAGE_SET_JSON_BYTES {
            return Err(AbiError::new(
                MC_STATUS_BOUNDS,
                format!(
                    "installed package-set JSON is {request_json_len} bytes, maximum is {MAX_INSTALLED_PACKAGE_SET_JSON_BYTES}"
                ),
            ));
        }
        let request_json =
            utf8_input(request_json, request_json_len, "installed package-set JSON")?;
        with_runtime_mut(handle, |runtime| {
            runtime.bootstrap_package_set(handle, generation, request_json, request_json_len)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_save_replay_json_v1(
    handle: u64,
    runtime_generation: u64,
    session_generation: u64,
    expected_tick: u64,
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    boundary(|| {
        with_runtime(handle, |runtime| {
            let bytes = runtime.save_replay(
                handle,
                runtime_generation,
                session_generation,
                expected_tick,
            )?;
            write_replay_save(&bytes, buffer, capacity, out_length)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_load_replay_json_v1(
    handle: u64,
    generation: u64,
    package_set_json: *const u8,
    package_set_json_len: u32,
    save_json: *const u8,
    save_json_len: u32,
) -> i32 {
    boundary(|| {
        if package_set_json.is_null() && package_set_json_len != 0 {
            return Err(AbiError::new(
                MC_STATUS_NULL_POINTER,
                "replay package-set JSON pointer is null",
            ));
        }
        if save_json.is_null() && save_json_len != 0 {
            return Err(AbiError::new(
                MC_STATUS_NULL_POINTER,
                "replay save JSON pointer is null",
            ));
        }
        if package_set_json_len > MAX_INSTALLED_PACKAGE_SET_JSON_BYTES {
            return Err(AbiError::new(
                MC_STATUS_BOUNDS,
                "replay package-set JSON exceeds the negotiated bound",
            ));
        }
        if usize::try_from(save_json_len).unwrap_or(usize::MAX) > MAX_MC2_CAMPAIGN_REPLAY_SAVE_BYTES
        {
            return Err(AbiError::new(
                MC_STATUS_BOUNDS,
                "replay save JSON exceeds the negotiated bound",
            ));
        }
        let package_set_json = utf8_input(
            package_set_json,
            package_set_json_len,
            "replay package-set JSON",
        )?;
        let save_json = utf8_input(save_json, save_json_len, "replay save JSON")?;
        with_runtime_mut(handle, |runtime| {
            runtime.load_replay(
                handle,
                generation,
                package_set_json,
                package_set_json_len,
                save_json.as_bytes(),
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_open_level_v1(handle: u64, generation: u64) -> i32 {
    boundary(|| with_runtime_mut(handle, |runtime| runtime.open_level(handle, generation)))
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_restart_new_game_v1(handle: u64, generation: u64) -> i32 {
    boundary(|| {
        with_runtime_mut(handle, |runtime| {
            runtime.restart_new_game(handle, generation)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_pause_v1(handle: u64, generation: u64) -> i32 {
    boundary(|| with_runtime_mut(handle, |runtime| runtime.pause(handle, generation)))
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_paused_frame_v1(
    handle: u64,
    runtime_generation: u64,
    session_generation: u64,
    expected_tick: u64,
) -> i32 {
    boundary(|| {
        with_runtime_mut(handle, |runtime| {
            runtime.paused_frame(
                handle,
                runtime_generation,
                session_generation,
                expected_tick,
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_resume_v1(handle: u64, generation: u64) -> i32 {
    boundary(|| with_runtime_mut(handle, |runtime| runtime.resume(handle, generation)))
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_close_v1(
    handle: u64,
    runtime_generation: u64,
    session_generation: u64,
    expected_tick: u64,
) -> i32 {
    boundary(|| {
        with_runtime_mut(handle, |runtime| {
            runtime.close(
                handle,
                runtime_generation,
                session_generation,
                expected_tick,
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_destroy_v1(handle: u64) -> i32 {
    boundary(|| {
        let entry = runtime_arc(handle)?;
        let mut guard = entry
            .lock()
            .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "game runtime lock is poisoned"))?;
        match &mut *guard {
            super::game_session_abi::NeutralSession::Runtime(runtime) if !runtime.closed => {
                runtime.closed = true;
                runtime.resolver = None;
            }
            _ => {
                return Err(AbiError::new(
                    MC_STATUS_UNKNOWN_HANDLE,
                    format!("unknown game runtime handle {handle}"),
                ));
            }
        }
        with_registry_mut(|registry| registry.remove_neutral(handle))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_v2::game_session_abi::{
        mc_game_session_destroy_v1, mc_game_session_reset_v1, mc_game_session_snapshot_json_v1,
        mc_game_session_step_json_v1,
    };
    use serde_json::Value;
    use std::path::PathBuf;

    fn level_one_package() -> PathBuf {
        std::env::var_os("MC_CAMPAIGN_LEVEL1_PACKAGE").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf15-level1-package-v3-current")
            },
            PathBuf::from,
        )
    }

    fn level_two_package() -> PathBuf {
        std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2")
            },
            PathBuf::from,
        )
    }

    fn level_three_package() -> PathBuf {
        std::env::var_os("MC_CAMPAIGN_PACKAGE_LEVEL3_V1").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf14-campaign-package-v1-a/qualified-level3")
            },
            PathBuf::from,
        )
    }

    fn installed_package_set_json(packages: &[PathBuf]) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "request_contract": INSTALLED_PACKAGE_SET_CONTRACT,
            "packages": packages
                .iter()
                .map(|package| serde_json::json!({
                    "package_dir": package,
                    "owner_cache_path": null
                }))
                .collect::<Vec<_>>()
        }))
        .unwrap()
    }

    fn bootstrap_package_set(handle: u64, generation: u64, packages: &[PathBuf]) -> i32 {
        let request = installed_package_set_json(packages);
        mc_game_runtime_bootstrap_package_set_json_v1(
            handle,
            generation,
            request.as_ptr(),
            request.len() as u32,
        )
    }

    fn copy_package_for_runtime_failure(source: &Path) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let destination = std::env::temp_dir().join(format!(
            "carpet-flight-runtime-open-failure-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&destination).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            assert!(entry.file_type().unwrap().is_file());
            fs::copy(entry.path(), destination.join(entry.file_name())).unwrap();
        }
        destination
    }

    fn read_json(read: impl Fn(*mut u8, u32, *mut u32) -> i32) -> Value {
        let mut length = 0_u32;
        assert_eq!(
            read(std::ptr::null_mut(), 0, &raw mut length),
            MC_STATUS_BOUNDS
        );
        let mut bytes = vec![0_u8; usize::try_from(length).unwrap()];
        assert_eq!(
            read(bytes.as_mut_ptr(), length, &raw mut length),
            MC_STATUS_OK
        );
        serde_json::from_slice(&bytes).unwrap()
    }

    fn read_bytes(read: impl Fn(*mut u8, u32, *mut u32) -> i32) -> Vec<u8> {
        let mut length = 0_u32;
        assert_eq!(
            read(std::ptr::null_mut(), 0, &raw mut length),
            MC_STATUS_BOUNDS
        );
        let mut bytes = vec![0_u8; usize::try_from(length).unwrap()];
        assert_eq!(
            read(bytes.as_mut_ptr(), length, &raw mut length),
            MC_STATUS_OK
        );
        bytes
    }

    fn identity(handle: u64) -> Value {
        read_json(|buffer, capacity, length| {
            mc_game_runtime_identity_json_v1(handle, buffer, capacity, length)
        })
    }

    #[test]
    fn runtime_resolver_owned_package_set_bootstraps_real_entry_on_same_handle() {
        let contract = read_json(|buffer, capacity, length| {
            mc_game_runtime_contract_json_v1(buffer, capacity, length)
        });
        assert_eq!(
            contract["runtime_contract"],
            "carpet-flight-game-runtime/v9"
        );
        assert_eq!(
            contract["transport_contract"],
            "carpet-flight-native-game-runtime-json/v8"
        );
        assert_eq!(contract["transport_version"], 8);
        assert_eq!(
            contract["modes"],
            serde_json::json!(["title", "loading", "playing", "paused"])
        );
        assert_eq!(
            contract["transitions"],
            serde_json::json!([
                "bootstrap_package_set",
                "restart_new_game",
                "open_level",
                "load_replay_save",
                "pause",
                "paused_frame",
                "resume",
                "close"
            ])
        );
        assert_eq!(
            contract["playing_session_transport"],
            "mc_game_session_*_v1 on the same handle"
        );
        assert_eq!(
            contract["installed_package_set"],
            serde_json::json!({
                "request_contract": "carpet-flight-installed-package-set-bootstrap/v1",
                "transport": "mc_game_runtime_bootstrap_package_set_json_v1",
                "selection_authority": "rust_owned_semantic_flow",
                "input_order": "unordered_resource_locations",
                "lifetime": "immutable_until_destroy"
            })
        );
        assert_eq!(
            contract["limits"]["maximum_installed_package_set_json_bytes"],
            64 * 1024
        );
        assert_eq!(contract["limits"]["maximum_installed_package_count"], 16);
        assert_eq!(
            contract["limits"]["maximum_replay_save_json_bytes"],
            16 * 1024 * 1024
        );
        assert_eq!(
            contract["replay_save"],
            serde_json::json!({
                "save_contract": "carpet-flight-mc2-campaign-replay-save/v4",
                "save_transport": "mc_game_runtime_save_replay_json_v1",
                "load_transport": "mc_game_runtime_load_replay_json_v1",
                "load_package_authority": "complete_unordered_resource_set",
                "client_semantics": "opaque_utf8_json"
            })
        );
        assert_eq!(
            contract["paused_frame"],
            serde_json::json!({
                "transport": "mc_game_runtime_paused_frame_v1",
                "gameplay_tick": "frozen",
                "session_identity": "unchanged",
                "deterministic_work": "package_declared_pre_gate",
                "replay": "ordered_operation"
            })
        );
        assert_eq!(
            contract["runtime_flow"],
            serde_json::json!({
                "title": "absent",
                "loading": "pending_definition.runtime_flow",
                "playing_and_paused": "active_runtime_flow"
            })
        );

        let mut handle = 0_u64;
        assert_eq!(mc_game_runtime_create_v1(&raw mut handle), MC_STATUS_OK);
        assert_ne!(handle, 0);
        let title = identity(handle);
        assert_eq!(title["mode"], "title");
        assert_eq!(title["generation"], "1");
        assert!(title.get("active_runtime_flow").is_none());

        let forbidden_next = serde_json::to_vec(&serde_json::json!({
            "request_contract": INSTALLED_PACKAGE_SET_CONTRACT,
            "packages": [{
                "package_dir": level_one_package(),
                "owner_cache_path": null,
                "next_package_dir": level_two_package()
            }]
        }))
        .unwrap();
        assert_eq!(
            mc_game_runtime_bootstrap_package_set_json_v1(
                handle,
                1,
                forbidden_next.as_ptr(),
                forbidden_next.len() as u32,
            ),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(identity(handle), title);

        let incomplete = installed_package_set_json(&[level_two_package(), level_one_package()]);
        assert_eq!(
            mc_game_runtime_bootstrap_package_set_json_v1(
                handle,
                1,
                incomplete.as_ptr(),
                incomplete.len() as u32,
            ),
            MC_STATUS_VALIDATION_FAILED
        );
        assert_eq!(identity(handle), title);

        let packages = installed_package_set_json(&[
            level_three_package(),
            level_one_package(),
            level_two_package(),
        ]);
        assert_eq!(
            mc_game_runtime_bootstrap_package_set_json_v1(
                handle,
                1,
                packages.as_ptr(),
                packages.len() as u32,
            ),
            MC_STATUS_OK
        );
        drop(packages);
        let installed_resolver = with_runtime(handle, |runtime| {
            runtime.resolver.clone().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_INTERNAL_PANIC,
                    "successful bootstrap did not retain its resolver",
                )
            })
        })
        .unwrap();
        let loading = identity(handle);
        assert_eq!(loading["mode"], "loading");
        assert_eq!(loading["generation"], "2");
        assert_eq!(
            loading["pending_definition"]["runtime_flow"]["game_id"],
            "game.magic_carpet_2"
        );
        assert_eq!(
            loading["pending_definition"]["runtime_flow"]["bound_level_id"],
            "level.magic_carpet_2.night_spire"
        );
        assert!(loading.get("active_runtime_flow").is_none());
        assert_eq!(mc_game_runtime_open_level_v1(handle, 2), MC_STATUS_OK);
        let playing = identity(handle);
        assert_eq!(playing["mode"], "playing");
        assert_eq!(playing["generation"], "3");
        assert_eq!(playing["session"]["session_id"], handle.to_string());
        assert_eq!(playing["session"]["generation"], "1");
        assert_eq!(playing["session"]["tick"], "0");
        assert_eq!(
            playing["active_runtime_flow"],
            loading["pending_definition"]["runtime_flow"]
        );
        assert_eq!(
            loading["pending_definition"]["package_graph_hash"],
            playing["session"]["package_graph_hash"]
        );
        assert_eq!(
            loading["pending_definition"]["definition_hash"],
            playing["session"]["definition_hash"]
        );
        assert_eq!(
            loading["pending_definition"]["definition_revision"],
            playing["session"]["definition_revision"]
        );

        let snapshot = read_json(|buffer, capacity, length| {
            mc_game_session_snapshot_json_v1(handle, buffer, capacity, length)
        });
        assert_eq!(snapshot["identity"], playing["session"]);
        let command = br#"{"longitudinal":"accelerate"}"#;
        assert_eq!(
            mc_game_session_step_json_v1(handle, 1, 0, command.as_ptr(), command.len() as u32,),
            MC_STATUS_OK
        );
        let stepped = read_json(|buffer, capacity, length| {
            mc_game_session_snapshot_json_v1(handle, buffer, capacity, length)
        });
        assert_eq!(stepped["identity"]["tick"], "1");

        assert_eq!(
            mc_game_session_destroy_v1(handle),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(identity(handle)["mode"], "playing");
        assert_eq!(mc_game_runtime_destroy_v1(handle), MC_STATUS_OK);
        let mut length = 0_u32;
        assert_eq!(
            mc_game_session_snapshot_json_v1(handle, std::ptr::null_mut(), 0, &raw mut length,),
            MC_STATUS_UNKNOWN_HANDLE
        );
        assert_eq!(mc_game_runtime_destroy_v1(handle), MC_STATUS_UNKNOWN_HANDLE);

        let mut returned_title = RuntimeRecord {
            runtime: GameRuntime::title(991).unwrap(),
            resolver: Some(installed_resolver),
            closed: false,
        };
        let returned_title_before = returned_title.runtime.identity();
        let replacement = returned_title
            .bootstrap_package_set(991, 1, "not-json".to_owned(), 8)
            .unwrap_err();
        assert_eq!(replacement.status, MC_STATUS_INVALID_ARGUMENT);
        assert!(
            replacement
                .message
                .contains("PACKAGE_SET_ALREADY_INSTALLED")
        );
        assert_eq!(returned_title.runtime.identity(), returned_title_before);
        assert!(returned_title.resolver.is_some());

        // No-Claim: this proves only complete-set admission and resolver-derived
        // entry opening. It does not expose completion acceptance, prove later
        // L2/L3 materialization, discover/install packages, migrate production
        // presentation, or implement paused frames, save/load, replay, or UI.
    }

    #[test]
    fn runtime_paused_frames_round_trip_real_level_one_across_fresh_handle() {
        let package_paths = [
            level_three_package(),
            level_one_package(),
            level_two_package(),
        ];
        let package_set = installed_package_set_json(&package_paths);

        let mut source = 0_u64;
        assert_eq!(mc_game_runtime_create_v1(&raw mut source), MC_STATUS_OK);
        assert_eq!(
            mc_game_runtime_bootstrap_package_set_json_v1(
                source,
                1,
                package_set.as_ptr(),
                package_set.len() as u32,
            ),
            MC_STATUS_OK
        );
        assert_eq!(mc_game_runtime_open_level_v1(source, 2), MC_STATUS_OK);
        for (tick, command) in [
            br#"{}"#.as_slice(),
            br#"{"longitudinal":"accelerate","lateral":"positive","pitch_axis":3}"#.as_slice(),
            br#"{"look_yaw":64,"look_pitch":-32}"#.as_slice(),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                mc_game_session_step_json_v1(
                    source,
                    1,
                    tick as u64,
                    command.as_ptr(),
                    command.len() as u32,
                ),
                MC_STATUS_OK
            );
        }
        assert_eq!(mc_game_runtime_pause_v1(source, 3), MC_STATUS_OK);
        let source_identity = identity(source);
        let before_paused_frames = read_json(|buffer, capacity, length| {
            mc_game_session_snapshot_json_v1(source, buffer, capacity, length)
        });
        assert_eq!(source_identity["mode"], "paused");
        assert_eq!(source_identity["session"]["tick"], "3");
        assert_eq!(
            mc_game_runtime_paused_frame_v1(source, 3, 2, 3),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(identity(source), source_identity);
        assert_eq!(
            mc_game_runtime_paused_frame_v1(source, 4, 1, 3),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(identity(source), source_identity);
        assert_eq!(
            mc_game_runtime_paused_frame_v1(source, 4, 2, 3),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_game_runtime_paused_frame_v1(source, 4, 2, 3),
            MC_STATUS_OK
        );
        let source_snapshot = read_json(|buffer, capacity, length| {
            mc_game_session_snapshot_json_v1(source, buffer, capacity, length)
        });
        assert_eq!(
            source_snapshot["identity"],
            before_paused_frames["identity"]
        );
        assert_ne!(
            source_snapshot["value"]["state_hash"],
            before_paused_frames["value"]["state_hash"]
        );
        assert_eq!(
            source_snapshot["value"]["terrain_hash"],
            before_paused_frames["value"]["terrain_hash"]
        );
        let save = read_bytes(|buffer, capacity, length| {
            mc_game_runtime_save_replay_json_v1(source, 4, 2, 3, buffer, capacity, length)
        });
        assert_eq!(identity(source), source_identity);
        assert_eq!(
            read_json(|buffer, capacity, length| {
                mc_game_session_snapshot_json_v1(source, buffer, capacity, length)
            }),
            source_snapshot
        );
        let save_json: Value = serde_json::from_slice(&save).unwrap();
        assert_eq!(
            save_json["save_contract"],
            "carpet-flight-mc2-campaign-replay-save/v4"
        );
        let operations = save_json["operations"].as_array().unwrap();
        assert_eq!(operations.len(), 5);
        assert_eq!(operations[3]["operation"], "paused_frame");
        assert_eq!(operations[4]["operation"], "paused_frame");
        assert_eq!(save_json["paused"], true);

        let mut restored = 0_u64;
        assert_eq!(mc_game_runtime_create_v1(&raw mut restored), MC_STATUS_OK);
        let restored_title = identity(restored);
        let incomplete = installed_package_set_json(&[level_one_package(), level_two_package()]);
        assert_eq!(
            mc_game_runtime_load_replay_json_v1(
                restored,
                1,
                incomplete.as_ptr(),
                incomplete.len() as u32,
                save.as_ptr(),
                save.len() as u32,
            ),
            MC_STATUS_VALIDATION_FAILED
        );
        assert_eq!(identity(restored), restored_title);

        assert_eq!(
            mc_game_runtime_load_replay_json_v1(
                restored,
                1,
                package_set.as_ptr(),
                package_set.len() as u32,
                save.as_ptr(),
                save.len() as u32,
            ),
            MC_STATUS_OK
        );
        let restored_identity = identity(restored);
        assert_eq!(restored_identity["mode"], "paused");
        assert_eq!(restored_identity["generation"], "2");
        assert_eq!(
            restored_identity["session"]["session_id"],
            restored.to_string()
        );
        assert_eq!(restored_identity["session"]["generation"], "1");
        assert_eq!(restored_identity["session"]["tick"], "3");
        assert_eq!(
            restored_identity["active_runtime_flow"],
            source_identity["active_runtime_flow"]
        );
        let restored_snapshot = read_json(|buffer, capacity, length| {
            mc_game_session_snapshot_json_v1(restored, buffer, capacity, length)
        });
        assert_eq!(restored_snapshot["value"], source_snapshot["value"]);
        let neutral = br#"{}"#;
        assert_eq!(
            mc_game_session_step_json_v1(restored, 1, 3, neutral.as_ptr(), neutral.len() as u32,),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(identity(restored), restored_identity);
        assert_eq!(mc_game_runtime_resume_v1(restored, 2), MC_STATUS_OK);
        assert_eq!(identity(restored)["mode"], "playing");
        assert_eq!(identity(restored)["session"]["generation"], "2");
        assert_eq!(
            mc_game_session_step_json_v1(restored, 2, 3, neutral.as_ptr(), neutral.len() as u32,),
            MC_STATUS_OK
        );
        assert_eq!(identity(restored)["session"]["tick"], "4");

        assert_eq!(mc_game_runtime_destroy_v1(source), MC_STATUS_OK);
        assert_eq!(mc_game_runtime_destroy_v1(restored), MC_STATUS_OK);

        // This proves two ordered real-L1 paused-frame services cross the
        // native ABI, survive an opaque replay save on a fresh resolver-owned
        // handle, preserve the gameplay tick, and reproduce the authoritative
        // snapshot. It does not claim later-level pause policy or save UI.
    }

    #[test]
    fn runtime_pause_resume_freezes_same_handle_campaign_session() {
        let mut handle = 0_u64;
        assert_eq!(mc_game_runtime_create_v1(&raw mut handle), MC_STATUS_OK);
        assert_eq!(
            bootstrap_package_set(
                handle,
                1,
                &[
                    level_three_package(),
                    level_one_package(),
                    level_two_package()
                ],
            ),
            MC_STATUS_OK
        );
        assert_eq!(mc_game_runtime_open_level_v1(handle, 2), MC_STATUS_OK);
        let playing = identity(handle);
        let before = read_json(|buffer, capacity, length| {
            mc_game_session_snapshot_json_v1(handle, buffer, capacity, length)
        });

        assert_eq!(
            mc_game_runtime_pause_v1(handle, 2),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(identity(handle), playing);
        assert_eq!(
            read_json(|buffer, capacity, length| {
                mc_game_session_snapshot_json_v1(handle, buffer, capacity, length)
            }),
            before
        );

        assert_eq!(mc_game_runtime_pause_v1(handle, 3), MC_STATUS_OK);
        let paused = identity(handle);
        assert_eq!(paused["mode"], "paused");
        assert_eq!(paused["generation"], "4");
        assert_eq!(paused["session"]["session_id"], handle.to_string());
        assert_eq!(paused["session"]["generation"], "2");
        assert_eq!(paused["session"]["tick"], "0");
        assert_eq!(
            paused["active_runtime_flow"],
            playing["active_runtime_flow"]
        );
        assert_eq!(
            paused["session"]["definition_hash"],
            playing["session"]["definition_hash"]
        );
        let paused_snapshot = read_json(|buffer, capacity, length| {
            mc_game_session_snapshot_json_v1(handle, buffer, capacity, length)
        });
        assert_eq!(paused_snapshot["identity"], paused["session"]);
        assert_eq!(paused_snapshot["value"], before["value"]);

        let command = br#"{"longitudinal":"accelerate"}"#;
        assert_eq!(
            mc_game_session_step_json_v1(handle, 2, 0, command.as_ptr(), command.len() as u32,),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            mc_game_session_reset_v1(handle, 2, 0),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(identity(handle), paused);
        assert_eq!(
            read_json(|buffer, capacity, length| {
                mc_game_session_snapshot_json_v1(handle, buffer, capacity, length)
            }),
            paused_snapshot
        );

        assert_eq!(mc_game_runtime_resume_v1(handle, 4), MC_STATUS_OK);
        let resumed = identity(handle);
        assert_eq!(resumed["mode"], "playing");
        assert_eq!(resumed["generation"], "5");
        assert_eq!(resumed["session"]["generation"], "3");
        assert_eq!(resumed["session"]["tick"], "0");
        assert_eq!(
            resumed["active_runtime_flow"],
            playing["active_runtime_flow"]
        );
        assert_eq!(
            read_json(|buffer, capacity, length| {
                mc_game_session_snapshot_json_v1(handle, buffer, capacity, length)
            })["value"],
            before["value"]
        );

        assert_eq!(
            mc_game_session_step_json_v1(handle, 2, 0, command.as_ptr(), command.len() as u32,),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(
            mc_game_session_step_json_v1(handle, 3, 0, command.as_ptr(), command.len() as u32,),
            MC_STATUS_OK
        );
        assert_eq!(
            read_json(|buffer, capacity, length| {
                mc_game_session_snapshot_json_v1(handle, buffer, capacity, length)
            })["identity"]["tick"],
            "1"
        );
        assert_eq!(mc_game_runtime_destroy_v1(handle), MC_STATUS_OK);

        // No-Claim: this transport exposes only a quiescent pause boundary.
        // It has no paused_frame export and therefore proves no MC2 pre-gate
        // RNG/objective/presentation/audio work, replay-visible paused time,
        // L2/L3 pause policy, or application pause UI.
    }

    #[test]
    fn runtime_close_returns_real_playing_session_to_title_on_the_same_handle() {
        let mut handle = 0_u64;
        assert_eq!(mc_game_runtime_create_v1(&raw mut handle), MC_STATUS_OK);
        assert_eq!(
            bootstrap_package_set(
                handle,
                1,
                &[
                    level_three_package(),
                    level_one_package(),
                    level_two_package()
                ],
            ),
            MC_STATUS_OK
        );
        assert_eq!(mc_game_runtime_open_level_v1(handle, 2), MC_STATUS_OK);
        let playing = identity(handle);
        let snapshot = read_json(|buffer, capacity, length| {
            mc_game_session_snapshot_json_v1(handle, buffer, capacity, length)
        });

        assert_eq!(
            mc_game_runtime_close_v1(handle, 3, 1, 1),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(identity(handle), playing);
        assert_eq!(
            read_json(|buffer, capacity, length| {
                mc_game_session_snapshot_json_v1(handle, buffer, capacity, length)
            }),
            snapshot
        );

        assert_eq!(mc_game_runtime_close_v1(handle, 3, 1, 0), MC_STATUS_OK);
        let title = identity(handle);
        assert_eq!(title["runtime_id"], handle.to_string());
        assert_eq!(title["generation"], "4");
        assert_eq!(title["mode"], "title");
        assert!(title.get("pending_definition").is_none());
        assert!(title.get("active_runtime_flow").is_none());
        assert!(title.get("session").is_none());
        let mut length = 0_u32;
        assert_eq!(
            mc_game_session_snapshot_json_v1(handle, std::ptr::null_mut(), 0, &raw mut length,),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            mc_game_runtime_close_v1(handle, 3, 1, 0),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(identity(handle), title);

        assert_eq!(mc_game_runtime_restart_new_game_v1(handle, 4), MC_STATUS_OK);
        let loading = identity(handle);
        assert_eq!(loading["runtime_id"], handle.to_string());
        assert_eq!(loading["generation"], "5");
        assert_eq!(loading["mode"], "loading");
        assert_eq!(
            loading["pending_definition"]["runtime_flow"]["bound_level_id"],
            "level.magic_carpet_2.night_spire"
        );
        assert_eq!(mc_game_runtime_open_level_v1(handle, 5), MC_STATUS_OK);
        let restarted = identity(handle);
        assert_eq!(restarted["runtime_id"], handle.to_string());
        assert_eq!(restarted["generation"], "6");
        assert_eq!(restarted["mode"], "playing");
        assert_eq!(restarted["session"]["session_id"], handle.to_string());
        assert_eq!(restarted["session"]["generation"], "2");
        assert_eq!(restarted["session"]["tick"], "0");
        assert_eq!(
            mc_game_session_step_json_v1(handle, 1, 0, b"{}".as_ptr(), 2),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(mc_game_runtime_destroy_v1(handle), MC_STATUS_OK);

        // No-Claim: close/restart proves a guarded same-handle return to Rust
        // Title and resolver-derived reopening of its immutable entry. It does
        // not choose another package, accept completion, present a menu, or
        // persist the closed state.
    }

    #[test]
    fn failed_runtime_transitions_preserve_identity_and_installed_resolver() {
        let mut handle = 0_u64;
        assert_eq!(mc_game_runtime_create_v1(&raw mut handle), MC_STATUS_OK);
        let title = identity(handle);
        assert_eq!(
            bootstrap_package_set(handle, 1, &[level_two_package(), level_one_package()],),
            MC_STATUS_VALIDATION_FAILED
        );
        assert_eq!(identity(handle), title);

        assert_eq!(
            mc_game_runtime_open_level_v1(handle, 2),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(identity(handle), title);

        let missing = level_one_package().join("missing-package-for-runtime-test");
        assert_eq!(
            bootstrap_package_set(handle, 2, &[missing]),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(identity(handle), title);

        assert_eq!(
            bootstrap_package_set(
                handle,
                1,
                &[
                    level_two_package(),
                    level_three_package(),
                    level_one_package()
                ],
            ),
            MC_STATUS_OK
        );
        let loading = identity(handle);

        assert_eq!(
            bootstrap_package_set(
                handle,
                2,
                &[
                    level_two_package(),
                    level_three_package(),
                    level_one_package()
                ],
            ),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(identity(handle), loading);

        assert_eq!(
            mc_game_runtime_open_level_v1(handle, 1),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(identity(handle), loading);
        assert_eq!(mc_game_runtime_open_level_v1(handle, 2), MC_STATUS_OK);
        assert_eq!(identity(handle)["mode"], "playing");
        assert_eq!(mc_game_runtime_destroy_v1(handle), MC_STATUS_OK);

        let mut failed_open_handle = 0_u64;
        assert_eq!(
            mc_game_runtime_create_v1(&raw mut failed_open_handle),
            MC_STATUS_OK
        );
        let failed_open_root = copy_package_for_runtime_failure(&level_one_package());
        assert_eq!(
            bootstrap_package_set(
                failed_open_handle,
                1,
                &[
                    level_two_package(),
                    level_three_package(),
                    failed_open_root.clone(),
                ],
            ),
            MC_STATUS_OK
        );
        let failed_open_loading = identity(failed_open_handle);
        assert_eq!(failed_open_loading["mode"], "loading");
        fs::remove_file(failed_open_root.join("runtime.bin")).unwrap();
        assert_eq!(
            mc_game_runtime_open_level_v1(failed_open_handle, 2),
            MC_STATUS_VALIDATION_FAILED
        );
        assert_eq!(identity(failed_open_handle), failed_open_loading);
        assert_eq!(
            mc_game_runtime_open_level_v1(failed_open_handle, 1),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(identity(failed_open_handle), failed_open_loading);
        assert_eq!(
            mc_game_runtime_open_level_v1(failed_open_handle, 2),
            MC_STATUS_VALIDATION_FAILED
        );
        assert_eq!(identity(failed_open_handle), failed_open_loading);
        assert_eq!(mc_game_runtime_destroy_v1(failed_open_handle), MC_STATUS_OK);
        fs::remove_dir_all(failed_open_root).unwrap();
    }
}
