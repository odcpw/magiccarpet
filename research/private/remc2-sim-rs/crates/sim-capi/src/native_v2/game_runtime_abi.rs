//! Rust-owned title/loading/playing runtime transport.
//!
//! A runtime remains behind one handle for its entire lifetime. Once it
//! reaches `playing`, the existing `mc_game_session_*_v1` exports operate on
//! the nested `GameSession`; this module does not create a second gameplay
//! authority or a parallel step API.

use super::*;
use sim_core::{
    campaign::backend::{
        Mc2CampaignAdapter, Mc2CampaignBackend, Mc2CampaignInputs, load_mc2_campaign,
    },
    game_definition::default_catalog,
    game_runtime::{
        GameRuntime, OpenLevelError, RuntimeGuard, RuntimeGuardError, RuntimeGuardErrorCode,
        RuntimeMode, RuntimeTransitionError,
    },
};
use sim_session::{GameSession, OpenSessionError};

const GAME_RUNTIME_TRANSPORT_CONTRACT: &str = "carpet-flight-native-game-runtime-json/v3";

pub(super) struct RuntimeRecord {
    runtime: GameRuntime<Mc2CampaignBackend>,
    pending_inputs: Option<Mc2CampaignInputs>,
    closed: bool,
}

impl RuntimeRecord {
    fn title(runtime_id: u64) -> AbiResult<Self> {
        let runtime = GameRuntime::title(runtime_id).map_err(map_transition_error)?;
        Ok(Self {
            runtime,
            pending_inputs: None,
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

    fn new_game(
        &mut self,
        handle: u64,
        generation: u64,
        inputs: Mc2CampaignInputs,
    ) -> AbiResult<()> {
        self.validate_transition(handle, generation, RuntimeMode::Title)?;
        let catalog = default_catalog().map_err(|error| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!("cannot construct game runtime semantic catalog: {error}"),
            )
        })?;
        let definition = load_mc2_campaign(&inputs.package_root, &catalog).map_err(|error| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!("cannot load game runtime campaign definition: {error}"),
            )
        })?;
        self.runtime
            .new_game(
                RuntimeGuard {
                    runtime_id: handle,
                    generation,
                },
                Arc::new(definition),
            )
            .map_err(map_transition_error)?;
        self.pending_inputs = Some(inputs);
        Ok(())
    }

    fn open_level(&mut self, handle: u64, generation: u64) -> AbiResult<()> {
        self.validate_transition(handle, generation, RuntimeMode::Loading)?;
        let inputs = self.pending_inputs.clone().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "MISSING_RUNTIME_INPUT: loading runtime has no validated campaign input",
            )
        })?;
        self.runtime
            .open_level(
                RuntimeGuard {
                    runtime_id: handle,
                    generation,
                },
                &Mc2CampaignAdapter,
                inputs,
                handle,
            )
            .map_err(map_open_level_error)?;
        self.pending_inputs = None;
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
    let status = match &error {
        RuntimeTransitionError::Guard(error)
            if matches!(error.code, RuntimeGuardErrorCode::GenerationMismatch) =>
        {
            MC_STATUS_GENERATION_MISMATCH
        }
        RuntimeTransitionError::InvalidRuntimeId | RuntimeTransitionError::Guard(_) => {
            MC_STATUS_UNKNOWN_HANDLE
        }
        RuntimeTransitionError::SessionGuard(_) => MC_STATUS_GENERATION_MISMATCH,
        RuntimeTransitionError::InvalidMode { .. } => MC_STATUS_INVALID_ARGUMENT,
        RuntimeTransitionError::InvalidRuntimeFlow { .. }
        | RuntimeTransitionError::InvalidNextDefinition { .. }
        | RuntimeTransitionError::InvalidCompletion { .. } => MC_STATUS_VALIDATION_FAILED,
        RuntimeTransitionError::CompletionNotReady => MC_STATUS_INVALID_ARGUMENT,
        RuntimeTransitionError::GenerationExhausted => MC_STATUS_GENERATION_MISMATCH,
    };
    AbiError::new(status, error.to_string())
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
            "transport_version": 3,
            "encoding": "utf-8-json",
            "u64_counter_encoding": "decimal-string",
            "modes": ["title", "loading", "playing", "paused"],
            "transitions": ["new_game", "open_level", "pause", "resume"],
            "playing_session_transport": "mc_game_session_*_v1 on the same handle",
            "package_profile": "mc2-campaign",
            "runtime_flow": {
                "title": "absent",
                "loading": "pending_definition.runtime_flow",
                "playing_and_paused": "active_runtime_flow"
            },
            "limits": {
                "maximum_output_json_bytes": super::game_session_abi::MAX_OUTPUT_JSON_BYTES,
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
pub extern "C" fn mc_game_runtime_new_game_v1(
    handle: u64,
    generation: u64,
    package_dir: *const u8,
    package_dir_len: u32,
    owner_cache_path: *const u8,
    owner_cache_path_len: u32,
) -> i32 {
    boundary(|| {
        let package = utf8_input(package_dir, package_dir_len, "package_dir")?;
        let owner_cache = utf8_input(owner_cache_path, owner_cache_path_len, "owner_cache_path")?;
        let inputs = campaign_inputs(package, owner_cache)?;
        with_runtime_mut(handle, |runtime| {
            runtime.new_game(handle, generation, inputs)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_open_level_v1(handle: u64, generation: u64) -> i32 {
    boundary(|| with_runtime_mut(handle, |runtime| runtime.open_level(handle, generation)))
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_pause_v1(handle: u64, generation: u64) -> i32 {
    boundary(|| with_runtime_mut(handle, |runtime| runtime.pause(handle, generation)))
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_resume_v1(handle: u64, generation: u64) -> i32 {
    boundary(|| with_runtime_mut(handle, |runtime| runtime.resume(handle, generation)))
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
                runtime.pending_inputs = None;
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
        std::env::var_os("MC_CAMPAIGN_LEVEL1_PACKAGE_V1").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf15-level1-package-current")
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

    fn utf8(path: &Path) -> Vec<u8> {
        path.to_string_lossy().as_bytes().to_vec()
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

    fn identity(handle: u64) -> Value {
        read_json(|buffer, capacity, length| {
            mc_game_runtime_identity_json_v1(handle, buffer, capacity, length)
        })
    }

    #[test]
    fn runtime_handle_transitions_and_drives_real_campaign_session() {
        let contract = read_json(|buffer, capacity, length| {
            mc_game_runtime_contract_json_v1(buffer, capacity, length)
        });
        assert_eq!(
            contract["runtime_contract"],
            "carpet-flight-game-runtime/v5"
        );
        assert_eq!(
            contract["transport_contract"],
            "carpet-flight-native-game-runtime-json/v3"
        );
        assert_eq!(contract["transport_version"], 3);
        assert_eq!(
            contract["modes"],
            serde_json::json!(["title", "loading", "playing", "paused"])
        );
        assert_eq!(
            contract["transitions"],
            serde_json::json!(["new_game", "open_level", "pause", "resume"])
        );
        assert_eq!(
            contract["playing_session_transport"],
            "mc_game_session_*_v1 on the same handle"
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

        let package = utf8(&level_one_package());
        assert_eq!(
            mc_game_runtime_new_game_v1(
                handle,
                1,
                package.as_ptr(),
                package.len() as u32,
                std::ptr::null(),
                0,
            ),
            MC_STATUS_OK
        );
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

        // No-Claim: this exposes definition-bound observational flow identity
        // only. It does not detect completion, resolve/select the next package,
        // accept transitions, execute Results/return-to-title, or save/load.
    }

    #[test]
    fn runtime_pause_resume_freezes_same_handle_campaign_session() {
        let mut handle = 0_u64;
        assert_eq!(mc_game_runtime_create_v1(&raw mut handle), MC_STATUS_OK);
        let package = utf8(&level_one_package());
        assert_eq!(
            mc_game_runtime_new_game_v1(
                handle,
                1,
                package.as_ptr(),
                package.len() as u32,
                std::ptr::null(),
                0,
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
    fn failed_runtime_transitions_preserve_identity_and_pending_binding() {
        let mut handle = 0_u64;
        assert_eq!(mc_game_runtime_create_v1(&raw mut handle), MC_STATUS_OK);
        let title = identity(handle);
        let package = utf8(&level_one_package());

        let later_package = utf8(&level_two_package());
        assert_eq!(
            mc_game_runtime_new_game_v1(
                handle,
                1,
                later_package.as_ptr(),
                later_package.len() as u32,
                std::ptr::null(),
                0,
            ),
            MC_STATUS_VALIDATION_FAILED
        );
        assert_eq!(identity(handle), title);

        assert_eq!(
            mc_game_runtime_open_level_v1(handle, 2),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(identity(handle), title);

        assert_eq!(
            mc_game_runtime_new_game_v1(
                handle,
                2,
                package.as_ptr(),
                package.len() as u32,
                std::ptr::null(),
                0,
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(identity(handle), title);

        assert_eq!(
            mc_game_runtime_new_game_v1(
                handle,
                1,
                package.as_ptr(),
                package.len() as u32,
                std::ptr::null(),
                0,
            ),
            MC_STATUS_OK
        );
        let loading = identity(handle);

        let missing_package = utf8(&level_one_package().join("missing-package-for-runtime-test"));
        assert_eq!(
            mc_game_runtime_new_game_v1(
                handle,
                2,
                missing_package.as_ptr(),
                missing_package.len() as u32,
                std::ptr::null(),
                0,
            ),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(identity(handle), loading);

        let level_two = utf8(&level_two_package());
        assert_eq!(
            mc_game_runtime_new_game_v1(
                handle,
                2,
                level_two.as_ptr(),
                level_two.len() as u32,
                std::ptr::null(),
                0,
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
        let failed_open_package = utf8(&failed_open_root);
        assert_eq!(
            mc_game_runtime_new_game_v1(
                failed_open_handle,
                1,
                failed_open_package.as_ptr(),
                failed_open_package.len() as u32,
                std::ptr::null(),
                0,
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
