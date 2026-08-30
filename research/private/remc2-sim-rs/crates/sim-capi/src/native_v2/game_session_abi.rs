//! Neutral, definition-bound session ABI.
//!
//! The legacy v2 ABI remains available for same-build compatibility
//! comparison. These additive exports own a distinct registry containing the
//! permanent `sim-core` `GameSession`; JSON is a bounded transport encoding of
//! domain values, never the authority or a client-side gameplay model.

use super::campaign_facade::SealedLevel1Backend;
use super::*;
use sim_core::{
    campaign::backend::{
        CampaignBackendError, Mc2CampaignAdapter, Mc2CampaignBackend, Mc2CampaignInputs,
        load_mc2_campaign,
    },
    game_definition::{
        BoundPresentationAsset, GameDefinition, Mc2OwnedInputs, SemanticId, WorkspaceStore,
        default_catalog, load_mc2_owned,
    },
};
use sim_session::{
    EventPage, GameAudioCommand, GameCommand, GameSession, GameSessionAssetError, GameSessionError,
    OpenSessionError, SessionGuard, SessionGuardError, SessionGuardErrorCode, SessionObservation,
};

const MAX_COMMAND_JSON_BYTES: u32 = 4 * 1024;
const MAX_ASSET_ID_BYTES: u32 = 256;
const MAX_EVENT_PAGE_SIZE: u32 = 1024;
pub(super) const MAX_OUTPUT_JSON_BYTES: u32 = 16 * 1024 * 1024;

/// A campaign descriptor package is selected by its own envelope, never by a
/// level-named symbol or open parameter.
const CAMPAIGN_DESCRIPTOR_ENVELOPE: &str = "campaign-definition.json";

/// One neutral session slot behind the shared `mc_game_session_*` handle
/// space. The adapter is selected once at open time from the validated
/// package shape; every later call dispatches internally, so both adapters
/// share one registry, one lifecycle, and one negotiated contract.
pub(super) enum NeutralSession {
    Mc2Owned(GameSession<SealedLevel1Backend>),
    Mc2Campaign(GameSession<Mc2CampaignBackend>),
    Runtime(super::game_runtime_abi::RuntimeRecord),
}

impl NeutralSession {
    /// Same-state compatibility projection for the frozen `mc_native_*`
    /// surfaces. The campaign adapter retains no `SessionRecord` carrier, so
    /// it fails closed instead of fabricating one.
    pub(super) fn legacy_projection(&self) -> AbiResult<&SessionRecord> {
        match self {
            Self::Mc2Owned(session) => Ok(session.backend().projection()),
            Self::Mc2Campaign(_) | Self::Runtime(_) => Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "MC2-owned compatibility projection is unavailable on a campaign-backed neutral session",
            )),
        }
    }

    pub(super) fn legacy_projection_mut(&mut self) -> AbiResult<&mut SessionRecord> {
        match self {
            Self::Mc2Owned(session) => Ok(session.backend_mut().projection_mut()),
            Self::Mc2Campaign(_) | Self::Runtime(_) => Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "MC2-owned compatibility projection is unavailable on a campaign-backed neutral session",
            )),
        }
    }

    fn write_snapshot(
        &self,
        buffer: *mut u8,
        capacity: u32,
        out_length: *mut u32,
    ) -> AbiResult<()> {
        match self {
            Self::Mc2Owned(session) => {
                write_json(&session.snapshot(), buffer, capacity, out_length)
            }
            Self::Mc2Campaign(session) => {
                write_json(&session.snapshot(), buffer, capacity, out_length)
            }
            Self::Runtime(runtime) => {
                write_json(&runtime.session()?.snapshot(), buffer, capacity, out_length)
            }
        }
    }

    fn step(&mut self, guard: SessionGuard, command: GameCommand) -> AbiResult<()> {
        match self {
            Self::Mc2Owned(session) => session
                .step(guard, command)
                .map(|_| ())
                .map_err(map_session_error),
            Self::Mc2Campaign(session) => session
                .step(guard, command)
                .map(|_| ())
                .map_err(map_campaign_session_error),
            Self::Runtime(runtime) => runtime
                .session_mut()?
                .step(guard, command)
                .map(|_| ())
                .map_err(map_campaign_session_error),
        }
    }

    fn reset(&mut self, guard: SessionGuard) -> AbiResult<()> {
        match self {
            Self::Mc2Owned(session) => session.reset(guard).map(|_| ()).map_err(map_session_error),
            Self::Mc2Campaign(session) => session
                .reset(guard)
                .map(|_| ())
                .map_err(map_campaign_session_error),
            Self::Runtime(runtime) => runtime
                .session_mut()?
                .reset(guard)
                .map(|_| ())
                .map_err(map_campaign_session_error),
        }
    }

    fn events_after(
        &self,
        guard: SessionGuard,
        cursor: u64,
        limit: usize,
    ) -> AbiResult<SessionObservation<EventPage>> {
        match self {
            Self::Mc2Owned(session) => session
                .events_after(guard, cursor, limit)
                .map_err(map_session_error),
            Self::Mc2Campaign(session) => session
                .events_after(guard, cursor, limit)
                .map_err(map_campaign_session_error),
            Self::Runtime(runtime) => runtime
                .session()?
                .events_after(guard, cursor, limit)
                .map_err(map_campaign_session_error),
        }
    }

    fn audio_for_event(
        &self,
        guard: SessionGuard,
        event_sequence: u64,
    ) -> AbiResult<SessionObservation<GameAudioCommand>> {
        match self {
            Self::Mc2Owned(session) => session
                .audio_for_event(guard, event_sequence)
                .map_err(map_session_error),
            Self::Mc2Campaign(session) => session
                .audio_for_event(guard, event_sequence)
                .map_err(map_campaign_session_error),
            Self::Runtime(runtime) => runtime
                .session()?
                .audio_for_event(guard, event_sequence)
                .map_err(map_campaign_session_error),
        }
    }

    fn presentation_asset(
        &self,
        guard: SessionGuard,
        id: &SemanticId,
    ) -> AbiResult<SessionObservation<BoundPresentationAsset>> {
        match self {
            Self::Mc2Owned(session) => session
                .presentation_asset(guard, id)
                .map_err(map_asset_error),
            Self::Mc2Campaign(session) => session
                .presentation_asset(guard, id)
                .map_err(map_asset_error),
            Self::Runtime(runtime) => runtime
                .session()?
                .presentation_asset(guard, id)
                .map_err(map_asset_error),
        }
    }
}

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

pub(super) fn map_guard_error(error: SessionGuardError) -> AbiError {
    let status = match error.code {
        SessionGuardErrorCode::GenerationMismatch | SessionGuardErrorCode::GenerationExhausted => {
            MC_STATUS_GENERATION_MISMATCH
        }
        SessionGuardErrorCode::InvalidSessionId | SessionGuardErrorCode::SessionMismatch => {
            MC_STATUS_UNKNOWN_HANDLE
        }
        SessionGuardErrorCode::TickMismatch => MC_STATUS_INVALID_ARGUMENT,
    };
    AbiError::new(status, error.to_string())
}

fn map_session_error(error: GameSessionError<AbiError>) -> AbiError {
    match error {
        GameSessionError::Backend(error) => error,
        GameSessionError::Guard(error) => map_guard_error(error),
    }
}

/// The campaign adapter's typed errors project onto the existing neutral
/// status vocabulary; no new status code or symbol is introduced. Unsupported
/// capabilities (positional audio) fail closed as bounded lookups.
pub(super) fn map_campaign_error(error: CampaignBackendError) -> AbiError {
    let status = match &error {
        CampaignBackendError::CursorBeyondHead { .. }
        | CampaignBackendError::NoAudioSidecar { .. } => MC_STATUS_BOUNDS,
        CampaignBackendError::CursorExpired { .. } => MC_STATUS_CURSOR_EXPIRED,
        CampaignBackendError::Open(_)
        | CampaignBackendError::Definition(_)
        | CampaignBackendError::UnappliedSemanticValue { .. }
        | CampaignBackendError::PackageSubstitution
        | CampaignBackendError::StepRejected { .. }
        | CampaignBackendError::JournalDesync => MC_STATUS_VALIDATION_FAILED,
    };
    AbiError::new(status, error.to_string())
}

fn map_campaign_session_error(error: GameSessionError<CampaignBackendError>) -> AbiError {
    match error {
        GameSessionError::Backend(error) => map_campaign_error(error),
        GameSessionError::Guard(error) => map_guard_error(error),
    }
}

fn map_asset_error(error: GameSessionAssetError) -> AbiError {
    match error {
        GameSessionAssetError::Definition(error) => AbiError::new(
            MC_STATUS_VALIDATION_FAILED,
            format!("cannot resolve neutral presentation asset: {error}"),
        ),
        GameSessionAssetError::Guard(error) => map_guard_error(error),
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
    with_registry_mut(|registry| {
        registry.insert_neutral(handle, NeutralSession::Mc2Owned(session))
    })?;
    Ok(handle)
}

fn open_neutral_campaign_session(
    inputs: Mc2CampaignInputs,
    definition: GameDefinition,
) -> AbiResult<u64> {
    let handle = next_session_id()?;
    let session = GameSession::open(&Mc2CampaignAdapter, Arc::new(definition), handle, inputs)
        .map_err(|error| match error {
            OpenSessionError::InvalidSessionId => AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "INVALID_SESSION_ID: session ID is zero",
            ),
            OpenSessionError::Adapter(error) => map_campaign_error(error),
        })?;
    with_registry_mut(|registry| {
        registry.insert_neutral(handle, NeutralSession::Mc2Campaign(session))
    })?;
    Ok(handle)
}

fn is_campaign_descriptor_package(package_dir: &str) -> bool {
    Path::new(package_dir)
        .join(CAMPAIGN_DESCRIPTOR_ENVELOPE)
        .is_file()
}

struct ExplicitOpenPaths {
    package: String,
    closure: String,
    cache: String,
}

fn explicit_open_paths(
    package_dir: *const u8,
    package_dir_len: u32,
    closure_path: *const u8,
    closure_path_len: u32,
    asset_cache_path: *const u8,
    asset_cache_path_len: u32,
) -> AbiResult<ExplicitOpenPaths> {
    Ok(ExplicitOpenPaths {
        package: utf8_input(package_dir, package_dir_len, "package_dir")?,
        closure: utf8_input(closure_path, closure_path_len, "closure_path")?,
        cache: utf8_input(asset_cache_path, asset_cache_path_len, "asset_cache_path")?,
    })
}

impl ExplicitOpenPaths {
    fn owner_inputs(&self) -> AbiResult<Mc2OwnedInputs> {
        if self.package.is_empty() || self.closure.is_empty() || self.cache.is_empty() {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "package_dir, closure_path, and asset_cache_path must be non-empty",
            ));
        }
        Ok(Mc2OwnedInputs::new(
            &self.package,
            &self.closure,
            &self.cache,
        ))
    }

    /// Validate the campaign open shape before any handle or session is
    /// allocated. A campaign descriptor package consumes no closure input;
    /// accepting one would silently bind the wrong backend.
    fn campaign_inputs(&self) -> AbiResult<Mc2CampaignInputs> {
        if !self.closure.is_empty() {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "campaign descriptor packages do not accept a closure path; pass an empty closure_path",
            ));
        }
        Ok(if self.cache.is_empty() {
            Mc2CampaignInputs::new(&self.package)
        } else {
            Mc2CampaignInputs::with_owner_cache(&self.package, &self.cache)
        })
    }
}

pub(super) fn write_json<T: serde::Serialize>(
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
            "engine_contract": sim_session::GAME_SESSION_CONTRACT,
            "transport_contract": "carpet-flight-native-json/v6",
            "transport_version": 6,
            "encoding": "utf-8-json",
            "u64_counter_encoding": "decimal-string",
            "u64_hash_encoding": "lowercase-hex-16",
            "capabilities": ["snapshot", "step", "reset", "events", "audio", "presentation_assets", "presentation_entities", "semantic_spell_selection"],
            "session_profiles": [
                {
                    "profile": "mc2-owned",
                    "snapshot_value": "game_snapshot/v1",
                    "capabilities": ["snapshot", "step", "reset", "events", "audio", "presentation_assets"]
                },
                {
                    "profile": "mc2-campaign",
                    "snapshot_value": "campaign_snapshot/v4",
                    "capabilities": ["snapshot", "step", "reset", "events", "presentation_assets", "presentation_entities", "semantic_spell_selection"]
                }
            ],
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
        let paths = explicit_open_paths(
            package_dir,
            package_dir_len,
            closure_path,
            closure_path_len,
            asset_cache_path,
            asset_cache_path_len,
        )?;
        if paths.package.is_empty() {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "package_dir must be non-empty",
            ));
        }
        let catalog = default_catalog().map_err(|error| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!("cannot construct neutral semantic catalog: {error}"),
            )
        })?;
        if is_campaign_descriptor_package(&paths.package) {
            let inputs = paths.campaign_inputs()?;
            let definition = load_mc2_campaign(&paths.package, &catalog).map_err(|error| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    format!("cannot load neutral campaign definition: {error}"),
                )
            })?;
            return open_neutral_campaign_session(inputs, definition);
        }
        let inputs = paths.owner_inputs()?;
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
        let paths = explicit_open_paths(
            package_dir,
            package_dir_len,
            closure_path,
            closure_path_len,
            asset_cache_path,
            asset_cache_path_len,
        )?;
        if is_campaign_descriptor_package(&paths.package) {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "campaign descriptor packages have no workspace-authored variant route yet; \
                 open the campaign package directly",
            ));
        }
        let inputs = paths.owner_inputs()?;
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
    boundary(|| {
        let session = neutral_session_arc(handle)?;
        let guard = session.lock().map_err(|_| {
            AbiError::new(MC_STATUS_LOCK_FAILED, "neutral session lock is poisoned")
        })?;
        if matches!(*guard, NeutralSession::Runtime(_)) {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "game runtime handles must be destroyed through mc_game_runtime_destroy_v1",
            ));
        }
        drop(guard);
        with_registry_mut(|registry| registry.remove_neutral(handle))
    })
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
            session.write_snapshot(buffer, capacity, out_length)
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
            session.step(session_guard(handle, generation, expected_tick), command)
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
            session.reset(session_guard(handle, generation, expected_tick))
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
            let page = session.events_after(
                session_guard(handle, generation, expected_tick),
                cursor,
                usize::try_from(limit)
                    .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "event limit exceeds usize"))?,
            )?;
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
            let command = session.audio_for_event(
                session_guard(handle, generation, expected_tick),
                event_sequence,
            )?;
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
        let semantic_id = SemanticId::parse(&semantic_id)
            .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error.to_string()))?;
        with_neutral_session(handle, |session| {
            let asset = session.presentation_asset(
                session_guard(handle, generation, expected_tick),
                &semantic_id,
            )?;
            write_json(&asset, buffer, capacity, out_length)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::game_definition::{
        ArtifactId, ChangeSetBuilder, IntegerParameter, PresentationAssetSlot, PublicationFault,
        RequestedTiming,
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
        read_json_with_capacity(16 * 1024, operation)
    }

    fn read_json_with_capacity(
        capacity: usize,
        operation: impl FnOnce(*mut u8, u32, *mut u32) -> i32,
    ) -> serde_json::Value {
        let mut bytes = vec![0_u8; capacity];
        let mut length = 0;
        assert_eq!(
            operation(bytes.as_mut_ptr(), bytes.len() as u32, &raw mut length),
            MC_STATUS_OK
        );
        serde_json::from_slice(&bytes[..length as usize]).unwrap()
    }

    fn campaign_fixture_path() -> PathBuf {
        std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2")
            },
            PathBuf::from,
        )
    }

    fn open_campaign_fixture() -> u64 {
        let package = campaign_fixture_path();
        open_campaign_path(&package)
    }

    fn open_campaign_path(package: &Path) -> u64 {
        let package = package.to_string_lossy();
        let mut handle = 0;
        assert_eq!(
            mc_game_session_open_v1(
                package.as_ptr(),
                package.len() as u32,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                &raw mut handle,
            ),
            MC_STATUS_OK
        );
        handle
    }

    fn campaign_snapshot(handle: u64) -> serde_json::Value {
        read_json_with_capacity(64 * 1024, |buffer, capacity, length| {
            mc_game_session_snapshot_json_v1(handle, buffer, capacity, length)
        })
    }

    fn native_last_error() -> String {
        let mut length = 0;
        assert_eq!(mc_native_last_error_length(&raw mut length), MC_STATUS_OK);
        let mut bytes = vec![0_u8; length as usize + 1];
        assert_eq!(
            mc_native_last_error_copy(
                bytes.as_mut_ptr(),
                u32::try_from(bytes.len()).unwrap(),
                &raw mut length,
            ),
            MC_STATUS_OK
        );
        String::from_utf8(bytes[..length as usize].to_vec()).unwrap()
    }

    fn level_one_autoplay_command(index: usize) -> Vec<u8> {
        let flags = if index < 360 { 1_u8 } else { 0 };
        let (roll_axis, pitch_axis) = if index < 145 {
            (38, 0)
        } else if (180..250).contains(&index) {
            (-32, -22)
        } else if (250..325).contains(&index) {
            (0, 24)
        } else {
            (0, 0)
        };
        serde_json::to_vec(&serde_json::json!({
            "longitudinal": if flags & 1 != 0 { "accelerate" } else { "coast" },
            "lateral": if (325..365).contains(&index) { "positive" } else { "neutral" },
            "roll_axis": roll_axis,
            "pitch_axis": pitch_axis,
            "cast_left": index == 160 || index == 300,
            "cast_right": false,
            "cast_selected": false,
            "look_yaw": 0,
            "look_pitch": 0,
        }))
        .expect("the bounded Level 1 command shape serializes")
    }

    fn common_gameplay_observation(snapshot: &serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "tick": snapshot["value"]["tick"],
            "player_pose": snapshot["value"]["player_pose"],
            "player_motion": snapshot["value"]["player_motion"],
        })
    }

    fn campaign_events(
        handle: u64,
        generation: u64,
        tick: u64,
        cursor: u64,
        limit: u32,
    ) -> serde_json::Value {
        read_json_with_capacity(4 * 1024 * 1024, |buffer, capacity, length| {
            mc_game_session_events_json_v1(
                handle, generation, tick, cursor, limit, buffer, capacity, length,
            )
        })
    }

    #[test]
    fn external_neutral_abi_opens_steps_pages_and_rejects_stale_or_unknown_commands() {
        let contract = read_json(|buffer, capacity, length| {
            mc_game_session_contract_json_v1(buffer, capacity, length)
        });
        assert_eq!(contract["engine_contract"], "carpet-flight-game-session/v6");
        assert_eq!(
            contract["transport_contract"],
            "carpet-flight-native-json/v6"
        );
        assert_eq!(contract["transport_version"], 6);
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
            .join("../sim-definition/tests/fixtures/custom-synthetic-v1");
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
        .bind_asset(
            PresentationAssetSlot::FireballProjectile,
            ArtifactId::parse("presentation.projectile_asset_variant").unwrap(),
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
        .set_integer(IntegerParameter::PlayerMaxForwardSpeed, 16)
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

    #[test]
    fn one_neutral_registry_serves_owned_and_campaign_adapters_through_one_contract() {
        let contract = read_json(|buffer, capacity, length| {
            mc_game_session_contract_json_v1(buffer, capacity, length)
        });
        let profiles = contract["session_profiles"].as_array().unwrap();
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0]["profile"], "mc2-owned");
        assert_eq!(profiles[0]["snapshot_value"], "game_snapshot/v1");
        assert_eq!(
            profiles[0]["capabilities"],
            serde_json::json!([
                "snapshot",
                "step",
                "reset",
                "events",
                "audio",
                "presentation_assets"
            ])
        );
        assert_eq!(profiles[1]["profile"], "mc2-campaign");
        assert_eq!(profiles[1]["snapshot_value"], "campaign_snapshot/v4");
        assert_eq!(
            profiles[1]["capabilities"],
            serde_json::json!([
                "snapshot",
                "step",
                "reset",
                "events",
                "presentation_assets",
                "presentation_entities",
                "semantic_spell_selection"
            ])
        );

        let owned = open_fixture();
        let campaign = open_campaign_fixture();
        assert_ne!(owned, campaign);
        let owned_initial = read_json(|buffer, capacity, length| {
            mc_game_session_snapshot_json_v1(owned, buffer, capacity, length)
        });

        let root = campaign_fixture_path();
        let mut direct = DescriptorCampaignSession::open(&root, CampaignSelection::LEVEL2).unwrap();
        // The direct authority mirrors the campaign route's entry binding so
        // hash equality below stays discriminating.
        direct
            .level2_compatibility_mut()
            .select_level2_campaign_entry_stage();
        // Journal-receipt equality against the direct authority is proven in
        // sim-core's campaign backend tests; this layer verifies the exact
        // transport encoding and its reset/replay determinism.
        fn assert_receipt_shape(snapshot: &serde_json::Value) {
            let receipt = snapshot["value"]["effect_journal_sha256"].as_str().unwrap();
            assert_eq!(receipt.len(), 64);
            assert!(
                receipt
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            );
        }

        let initial = campaign_snapshot(campaign);
        assert_receipt_shape(&initial);
        assert_eq!(initial["identity"]["session_id"], campaign.to_string());
        assert_eq!(initial["identity"]["generation"], "1");
        assert_eq!(initial["identity"]["tick"], "0");
        assert_eq!(initial["identity"]["definition_revision"], "1");
        assert_eq!(initial["value"]["tick"], "0");
        assert_eq!(initial["value"]["effect_head"], "0");
        assert_eq!(
            initial["value"]["objective_progress"],
            serde_json::json!({
                "current_objective": "objective.reach.payahandra_ruins",
                "completed_count": 0,
                "total_count": 5,
                "all_objectives_satisfied": false
            })
        );
        assert_eq!(
            initial["value"]["state_hash"],
            format!("{:016x}", direct.state_hash())
        );
        assert_eq!(
            initial["value"]["terrain_hash"],
            format!("{:016x}", direct.terrain_hash())
        );
        assert!(initial["value"].get("conformance_hash").is_none());
        assert!(initial["value"].get("caster_mana").is_none());
        let text = initial.to_string();
        for leaked in [
            "source_slot",
            "source_opcode",
            "class_id",
            "level1",
            "level2",
        ] {
            assert!(!text.contains(leaked), "campaign snapshot leaks {leaked}");
        }

        // Zero/one/many observational polls leave the authority in place.
        for _ in 0..3 {
            assert_eq!(campaign_snapshot(campaign), initial);
            let page = campaign_events(campaign, 1, 0, 0, 1_024);
            assert!(page["value"]["events"].as_array().unwrap().is_empty());
            assert!(!page["value"]["has_more"].as_bool().unwrap());
        }

        // The owned adapter still serves the same-build compatibility
        // projection; the campaign adapter fails closed instead of
        // fabricating a SessionRecord carrier.
        let mut compatibility = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_snapshot(
                owned,
                &raw mut compatibility,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_session_snapshot(
                campaign,
                &raw mut compatibility,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_INVALID_ARGUMENT
        );

        let command = br#"{"longitudinal":"accelerate"}"#;
        assert_eq!(
            mc_game_session_step_json_v1(campaign, 1, 0, command.as_ptr(), command.len() as u32),
            MC_STATUS_OK
        );
        let _ = direct.step(
            PlayerCommand {
                flags: 1,
                ..PlayerCommand::default()
            },
            0,
        );
        let stepped = campaign_snapshot(campaign);
        assert_eq!(stepped["identity"]["tick"], "1");
        assert_eq!(
            stepped["value"]["state_hash"],
            format!("{:016x}", direct.state_hash())
        );
        assert_receipt_shape(&stepped);
        assert_ne!(
            stepped["value"]["effect_journal_sha256"],
            initial["value"]["effect_journal_sha256"]
        );
        let head: u64 = stepped["value"]["effect_head"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        assert!(head > 0);

        // Stale tick and stale cursor fail without touching the journal.
        assert_eq!(
            mc_game_session_step_json_v1(campaign, 1, 0, command.as_ptr(), command.len() as u32),
            MC_STATUS_INVALID_ARGUMENT
        );
        let mut ignored = 0;
        assert_eq!(
            mc_game_session_events_json_v1(
                campaign,
                1,
                1,
                head + 1,
                1,
                std::ptr::null_mut(),
                0,
                &raw mut ignored,
            ),
            MC_STATUS_BOUNDS
        );
        assert_eq!(campaign_snapshot(campaign), stepped);

        // Bounded pages drain the same ordered rows as the full journal.
        let mut drained = Vec::new();
        let mut cursor = 0_u64;
        loop {
            let page = campaign_events(campaign, 1, 1, cursor, 1_024);
            assert_eq!(page["value"]["compatibility"], serde_json::Value::Null);
            let events = page["value"]["events"].as_array().unwrap();
            drained.extend(events.iter().cloned());
            if !page["value"]["has_more"].as_bool().unwrap() {
                break;
            }
            cursor = page["value"]["next_cursor"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap();
        }
        assert_eq!(drained.len() as u64, head);
        for (index, event) in drained.iter().enumerate() {
            assert_eq!(
                event["sequence"].as_str().unwrap().parse::<u64>().unwrap(),
                index as u64 + 1
            );
            let id = event["event_id"].as_str().unwrap();
            for leaked in ["level", "source", "slot", "class", "model"] {
                assert!(!id.contains(leaked), "{id} leaks compatibility vocabulary");
            }
        }
        let mut paged = Vec::new();
        let mut cursor = 0_u64;
        loop {
            let page = campaign_events(campaign, 1, 1, cursor, 3);
            let events = page["value"]["events"].as_array().unwrap();
            paged.extend(events.iter().cloned());
            if !page["value"]["has_more"].as_bool().unwrap() {
                break;
            }
            cursor = page["value"]["next_cursor"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap();
        }
        assert_eq!(paged, drained);

        // Audio and presentation remain unavailable for this real Level-2
        // package; profile capability does not fabricate package-absent data.
        let mut audio_length = 0;
        assert_eq!(
            mc_game_session_audio_json_v1(
                campaign,
                1,
                1,
                1,
                std::ptr::null_mut(),
                0,
                &raw mut audio_length,
            ),
            MC_STATUS_BOUNDS
        );
        assert_eq!(audio_length, 0);
        let slot = b"presentation.spell.fireball.projectile_asset";
        let mut asset_length = 0;
        assert_eq!(
            mc_game_session_presentation_asset_json_v1(
                campaign,
                1,
                1,
                slot.as_ptr(),
                slot.len() as u32,
                std::ptr::null_mut(),
                0,
                &raw mut asset_length,
            ),
            MC_STATUS_VALIDATION_FAILED
        );
        assert_eq!(campaign_snapshot(campaign), stepped);

        // Reset advances only the generation and restores the initial value;
        // a stale generation is rejected afterwards; replay is exact.
        assert_eq!(mc_game_session_reset_v1(campaign, 1, 1), MC_STATUS_OK);
        direct.reset();
        let reset = campaign_snapshot(campaign);
        assert_eq!(reset["identity"]["generation"], "2");
        assert_eq!(reset["identity"]["tick"], "0");
        assert_eq!(reset["value"], initial["value"]);
        assert_eq!(
            mc_game_session_events_json_v1(
                campaign,
                1,
                0,
                0,
                1,
                std::ptr::null_mut(),
                0,
                &raw mut ignored,
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(campaign_snapshot(campaign), reset);
        assert_eq!(
            mc_game_session_step_json_v1(campaign, 2, 0, command.as_ptr(), command.len() as u32),
            MC_STATUS_OK
        );
        let replay = campaign_snapshot(campaign);
        assert_eq!(replay["identity"]["generation"], "2");
        assert_eq!(replay["value"], stepped["value"]);

        // The owned session was never touched by any campaign operation.
        let owned_after = read_json(|buffer, capacity, length| {
            mc_game_session_snapshot_json_v1(owned, buffer, capacity, length)
        });
        assert_eq!(owned_after, owned_initial);

        // Destroy and reopen through the same symbol; the reopened session
        // replays the identical initial and stepped values.
        assert_eq!(mc_game_session_destroy_v1(campaign), MC_STATUS_OK);
        assert_eq!(
            mc_game_session_destroy_v1(campaign),
            MC_STATUS_UNKNOWN_HANDLE
        );
        let reopened = open_campaign_fixture();
        let reopened_initial = campaign_snapshot(reopened);
        assert_eq!(reopened_initial["value"], initial["value"]);
        assert_eq!(
            mc_game_session_step_json_v1(reopened, 1, 0, command.as_ptr(), command.len() as u32),
            MC_STATUS_OK
        );
        assert_eq!(campaign_snapshot(reopened)["value"], stepped["value"]);
        assert_eq!(mc_game_session_destroy_v1(reopened), MC_STATUS_OK);
        assert_eq!(mc_game_session_destroy_v1(owned), MC_STATUS_OK);
    }

    #[test]
    fn neutral_campaign_contract_opens_steps_and_resets_real_levels_one_two_and_three() {
        let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let packages = [
            generated.join("mc-cf15-level1-package-v3-current"),
            generated.join("mc-cf02-campaign-package-v2-b/qualified-level2"),
            generated.join("mc-cf14-campaign-package-v1-a/qualified-level3"),
        ];
        let command = br#"{"longitudinal":"accelerate"}"#;

        for package in packages {
            let handle = open_campaign_path(&package);
            let initial = campaign_snapshot(handle);
            assert_eq!(initial["identity"]["generation"], "1");
            assert_eq!(initial["identity"]["tick"], "0");
            assert_eq!(initial["value"]["tick"], "0");

            assert_eq!(
                mc_game_session_step_json_v1(handle, 1, 0, command.as_ptr(), command.len() as u32,),
                MC_STATUS_OK,
                "{} must step through the neutral campaign backend",
                package.display()
            );
            let stepped = campaign_snapshot(handle);
            assert_eq!(stepped["identity"]["tick"], "1");
            assert_eq!(stepped["value"]["tick"], "1");
            assert_ne!(
                stepped["value"]["state_hash"],
                initial["value"]["state_hash"]
            );

            assert_eq!(mc_game_session_reset_v1(handle, 1, 1), MC_STATUS_OK);
            let reset = campaign_snapshot(handle);
            assert_eq!(reset["identity"]["generation"], "2");
            assert_eq!(reset["identity"]["tick"], "0");
            assert_eq!(reset["value"], initial["value"]);
            assert_eq!(mc_game_session_destroy_v1(handle), MC_STATUS_OK);
        }
    }

    #[test]
    fn neutral_level_one_campaign_presentation_asset_query_is_identity_bound_and_observational() {
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-cf15-level1-package-v3-current");
        let handle = open_campaign_path(&package);
        let before = campaign_snapshot(handle);
        let slot = b"presentation.spell.fireball.projectile_asset";
        let asset = read_json_with_capacity(32 * 1024, |buffer, capacity, length| {
            mc_game_session_presentation_asset_json_v1(
                handle,
                1,
                0,
                slot.as_ptr(),
                slot.len() as u32,
                buffer,
                capacity,
                length,
            )
        });
        assert_eq!(asset["identity"], before["identity"]);
        assert_eq!(
            asset["value"]["content_address"],
            "sha256:331df08c5fa8483d5dd75dc10abd9dd76cc26918d6447403b3f3199bc31f521d"
        );
        assert_eq!(
            (
                asset["value"]["width"].as_u64(),
                asset["value"]["height"].as_u64()
            ),
            (Some(34), Some(35))
        );
        assert_eq!(asset["value"]["frame_count"], 1);
        assert_eq!(campaign_snapshot(handle), before);
        assert_eq!(mc_game_session_destroy_v1(handle), MC_STATUS_OK);
        // No-Claim: this proves only same-handle native observation of the
        // qualified Level-1 base frame. It does not prove authoring transport,
        // Unity rendering, animation, projectile gameplay, or Level 2/3 data.
    }

    #[test]
    fn neutral_level_one_campaign_snapshot_projects_the_live_semantic_fireball() {
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-cf15-level1-package-v3-current");
        let handle = open_campaign_path(&package);
        let initial = campaign_snapshot(handle);
        assert_eq!(
            initial["value"]["player_vitals"],
            serde_json::json!({
                "life": 10_000,
                "max_life": 10_000,
                "mana": 1_000,
                "max_mana": 1_000
            })
        );
        assert_eq!(
            initial["value"]["objective_progress"],
            serde_json::json!({
                "current_objective": "objective.reach.my_beacon",
                "completed_count": 0,
                "total_count": 5,
                "all_objectives_satisfied": false
            })
        );
        assert_eq!(
            initial["value"]["presentation_entities"],
            serde_json::json!([])
        );

        let command = br#"{"longitudinal":"coast","lateral":"neutral","roll_axis":0,"pitch_axis":0,"cast_left":true,"cast_right":false,"cast_selected":false,"look_yaw":0,"look_pitch":0}"#;
        assert_eq!(
            mc_game_session_step_json_v1(handle, 1, 0, command.as_ptr(), command.len() as u32,),
            MC_STATUS_OK
        );
        let launched = campaign_snapshot(handle);
        let entities = launched["value"]["presentation_entities"]
            .as_array()
            .expect("presentation entity array");
        assert_eq!(entities.len(), 1);
        let entity = &entities[0];
        assert_eq!(entity["kind"], "spell.fireball.projectile");
        assert_eq!(entity["logical_height"], 100);
        assert_eq!(
            entity["asset"],
            serde_json::json!({
                "id": "presentation.spell.fireball.projectile_asset",
                "content_address": "sha256:331df08c5fa8483d5dd75dc10abd9dd76cc26918d6447403b3f3199bc31f521d"
            })
        );
        assert!(entity["actor"].as_str().unwrap().parse::<u64>().unwrap() != 0);
        assert!(entity["pose"]["x"].is_u64());
        assert!(entity["pose"]["y"].is_u64());
        assert!(entity["pose"]["z"].is_i64() || entity["pose"]["z"].is_u64());

        let no_command = br#"{}"#;
        assert_eq!(
            mc_game_session_step_json_v1(
                handle,
                1,
                1,
                no_command.as_ptr(),
                no_command.len() as u32,
            ),
            MC_STATUS_OK
        );
        let charged = campaign_snapshot(handle);
        assert_eq!(charged["value"]["player_vitals"]["mana"], 900);
        assert_eq!(charged["value"]["player_vitals"]["life"], 10_000);
        assert_eq!(
            charged["value"]["objective_progress"],
            initial["value"]["objective_progress"]
        );

        assert_eq!(mc_game_session_reset_v1(handle, 1, 2), MC_STATUS_OK);
        let reset = campaign_snapshot(handle);
        assert_eq!(reset["identity"]["generation"], "2");
        assert_eq!(
            reset["value"]["player_vitals"],
            initial["value"]["player_vitals"]
        );
        assert_eq!(
            reset["value"]["presentation_entities"],
            serde_json::json!([])
        );
        assert_eq!(
            reset["value"]["objective_progress"],
            initial["value"]["objective_progress"]
        );
        assert_eq!(mc_game_session_destroy_v1(handle), MC_STATUS_OK);
        // No-Claim: this proves the real native campaign snapshot carries one
        // live semantic Fireball, observes the following-pass authoritative
        // mana charge, and restores frame-zero player vitals on reset. It does
        // not prove a Unity renderer, authored asset variants, animation,
        // player damage, or Level 2/3 data.
    }

    #[test]
    fn native_json_semantic_spell_selection_is_closed_and_atomic() {
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-cf15-level1-package-v3-current");
        let handle = open_campaign_path(&package);
        let initial = campaign_snapshot(handle);

        let unacquired = br#"{"select_spell":"spell.castle"}"#;
        assert_eq!(
            mc_game_session_step_json_v1(
                handle,
                1,
                0,
                unacquired.as_ptr(),
                unacquired.len() as u32,
            ),
            MC_STATUS_VALIDATION_FAILED
        );
        assert_eq!(
            campaign_snapshot(handle),
            initial,
            "an unacquired semantic selection must not advance or mutate the Rust session"
        );

        let select = br#"{"select_spell":"spell.fireball"}"#;
        let select_status =
            mc_game_session_step_json_v1(handle, 1, 0, select.as_ptr(), select.len() as u32);
        assert_eq!(select_status, MC_STATUS_OK, "{}", native_last_error());
        let selected = campaign_snapshot(handle);
        assert_eq!(selected["identity"]["tick"], "1");
        assert_eq!(
            selected["value"]["presentation_entities"],
            serde_json::json!([]),
            "selection alone does not manufacture a spell effect"
        );

        let cast = br#"{"cast_selected":true}"#;
        assert_eq!(
            mc_game_session_step_json_v1(handle, 1, 1, cast.as_ptr(), cast.len() as u32,),
            MC_STATUS_OK
        );
        let launched = campaign_snapshot(handle);
        assert_eq!(launched["identity"]["tick"], "2");
        assert!(
            launched["value"]["presentation_entities"]
                .as_array()
                .is_some_and(|entities| entities
                    .iter()
                    .any(|entity| { entity["kind"] == "spell.fireball.projectile" })),
            "the separately selected acquired semantic Fireball drives the ordinary cast path"
        );
        assert_eq!(mc_game_session_destroy_v1(handle), MC_STATUS_OK);
    }

    #[test]
    fn level_one_descriptor_and_sealed_backends_match_the_public_autoplay_observation() {
        let sealed = open_fixture();
        let descriptor_package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-cf15-level1-package-v3-current");
        let descriptor = open_campaign_path(&descriptor_package);
        let sealed_initial = campaign_snapshot(sealed);
        let descriptor_initial = campaign_snapshot(descriptor);
        assert_eq!(
            common_gameplay_observation(&descriptor_initial),
            common_gameplay_observation(&sealed_initial),
            "the two real Level 1 backends must start from the same public gameplay state"
        );

        for index in 0..440 {
            let command = level_one_autoplay_command(index);
            assert_eq!(
                mc_game_session_step_json_v1(
                    sealed,
                    1,
                    index as u64,
                    command.as_ptr(),
                    command.len() as u32,
                ),
                MC_STATUS_OK,
                "sealed Level 1 step {index}"
            );
            assert_eq!(
                mc_game_session_step_json_v1(
                    descriptor,
                    1,
                    index as u64,
                    command.as_ptr(),
                    command.len() as u32,
                ),
                MC_STATUS_OK,
                "descriptor Level 1 step {index}"
            );
            assert_eq!(
                common_gameplay_observation(&campaign_snapshot(descriptor)),
                common_gameplay_observation(&campaign_snapshot(sealed)),
                "Level 1 public gameplay diverged after command {index}"
            );
        }

        assert_eq!(mc_game_session_reset_v1(sealed, 1, 440), MC_STATUS_OK);
        assert_eq!(mc_game_session_reset_v1(descriptor, 1, 440), MC_STATUS_OK);
        assert_eq!(
            common_gameplay_observation(&campaign_snapshot(descriptor)),
            common_gameplay_observation(&campaign_snapshot(sealed))
        );
        assert_eq!(mc_game_session_destroy_v1(descriptor), MC_STATUS_OK);
        assert_eq!(mc_game_session_destroy_v1(sealed), MC_STATUS_OK);
    }

    #[test]
    fn campaign_package_open_rejects_wrong_shapes_before_neutral_allocation() {
        const SENTINEL: u64 = 0xfeed_face_dead_beef;
        let campaign = campaign_fixture_path();
        let campaign_text = campaign.to_string_lossy();
        let (owned_package, closure, cache) = fixture_paths();
        let owned_text = owned_package.to_string_lossy();
        let closure_text = closure.to_string_lossy();
        let cache_text = cache.to_string_lossy();

        // The shape error is typed and precedes definition or world loading.
        let error = ExplicitOpenPaths {
            package: campaign_text.to_string(),
            closure: closure_text.to_string(),
            cache: String::new(),
        }
        .campaign_inputs()
        .unwrap_err();
        assert_eq!(error.status, MC_STATUS_INVALID_ARGUMENT);
        assert!(error.message.contains("do not accept a closure path"));

        // A campaign package combined with an owner closure fails closed.
        let mut handle = SENTINEL;
        assert_eq!(
            mc_game_session_open_v1(
                campaign_text.as_ptr(),
                campaign_text.len() as u32,
                closure_text.as_ptr(),
                closure_text.len() as u32,
                std::ptr::null(),
                0,
                &raw mut handle,
            ),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(handle, SENTINEL);

        // The workspace route has no campaign variant yet and must not
        // allocate a handle or fall back to the owned adapter.
        let workspace = b"/must-not-open/neutral-campaign-workspace";
        assert_eq!(
            mc_game_session_open_workspace_v1(
                campaign_text.as_ptr(),
                campaign_text.len() as u32,
                closure_text.as_ptr(),
                closure_text.len() as u32,
                cache_text.as_ptr(),
                cache_text.len() as u32,
                workspace.as_ptr(),
                workspace.len() as u32,
                &raw mut handle,
            ),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(handle, SENTINEL);

        // An owned package with an empty closure keeps the exact existing
        // rejection instead of silently routing to the campaign adapter.
        assert_eq!(
            mc_game_session_open_v1(
                owned_text.as_ptr(),
                owned_text.len() as u32,
                std::ptr::null(),
                0,
                cache_text.as_ptr(),
                cache_text.len() as u32,
                &raw mut handle,
            ),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(handle, SENTINEL);

        // A tampered campaign envelope fails descriptor preflight; it never
        // selects a fallback backend.
        let bogus = std::env::temp_dir().join(format!(
            "carpet-flight-neutral-campaign-bogus-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&bogus);
        std::fs::create_dir_all(&bogus).unwrap();
        std::fs::write(bogus.join("campaign-definition.json"), b"{}").unwrap();
        std::fs::write(bogus.join("manifest.json"), b"{}").unwrap();
        let bogus_text = bogus.to_string_lossy();
        assert_eq!(
            mc_game_session_open_v1(
                bogus_text.as_ptr(),
                bogus_text.len() as u32,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                &raw mut handle,
            ),
            MC_STATUS_VALIDATION_FAILED
        );
        assert_eq!(handle, SENTINEL);
        let _ = std::fs::remove_dir_all(bogus);

        // The registry still serves a well-shaped campaign open afterwards.
        let opened = open_campaign_fixture();
        assert_eq!(mc_game_session_destroy_v1(opened), MC_STATUS_OK);
        assert_eq!(mc_game_session_destroy_v1(opened), MC_STATUS_UNKNOWN_HANDLE);
    }

    #[test]
    fn neutral_session_symbol_family_stays_frozen_and_level_free() {
        let source = include_str!("game_session_abi.rs");
        let exports = source
            .match_indices("pub extern \"C\" fn ")
            .map(|(index, pattern)| {
                source[index + pattern.len()..]
                    .split('(')
                    .next()
                    .unwrap()
                    .trim()
                    .to_owned()
            })
            .collect::<std::collections::BTreeSet<_>>();
        let frozen = [
            "mc_game_session_audio_json_v1",
            "mc_game_session_contract_json_v1",
            "mc_game_session_destroy_v1",
            "mc_game_session_events_json_v1",
            "mc_game_session_open_v1",
            "mc_game_session_open_workspace_v1",
            "mc_game_session_presentation_asset_json_v1",
            "mc_game_session_reset_v1",
            "mc_game_session_snapshot_json_v1",
            "mc_game_session_step_json_v1",
        ]
        .map(str::to_owned)
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            exports, frozen,
            "the neutral session symbol family is frozen; new adapters must \
             dispatch behind the existing handles instead of adding symbols"
        );
        for name in &exports {
            assert!(
                !name.contains("campaign") && !name.contains("level"),
                "{name} introduces a per-adapter or per-level symbol"
            );
        }
    }
}
