//! One local JSONL transport for the existing campaign runtime authority.
//! No gameplay, content resolution or replay implementation lives here.

use serde::Deserialize;
use serde_json::{Value, json};
use sim_core::campaign::backend::{
    CampaignBackendError, MAX_MC2_CAMPAIGN_REPLAY_SAVE_BYTES, Mc2CampaignBackend,
    Mc2CampaignInputs, Mc2CampaignReplaySaveError, Mc2RuntimePackageResolver,
};
use sim_core::game_definition::default_catalog;
use sim_core::game_runtime::{
    CampaignMenuCommand, GAME_RUNTIME_CONTRACT, GameRuntime, OpenLevelError, PausedFrameError,
    RuntimeGuard, RuntimeMode, RuntimeTransitionError,
};
use sim_core::game_session::{
    ActorPageRequest, CampaignSnapshot, GAME_SESSION_CONTRACT, GameCommand, GameSession,
    GameSessionError, SessionGuard, SessionObservation,
};
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

const CONTRACT: &str = "carpet-flight-command-stream/v1";
const MAX_REQUEST_BYTES: usize = 64 * 1024;
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_PAGE: usize = 100;
const MAX_PACKAGES: usize = 64;

pub(crate) fn capabilities() -> Value {
    json!({
        "transport": "stdin_stdout_jsonl", "contract": CONTRACT,
        "instances": "one_process_owned_runtime", "mutation": "guarded_runtime_and_session",
        "entry": "sim-cli game session",
        "first_request": {"contract": CONTRACT, "id": "1", "command": {"operation": "discover"}},
        "limits": {"request_bytes": MAX_REQUEST_BYTES, "response_bytes": MAX_RESPONSE_BYTES,
            "page_size": MAX_PAGE, "packages": MAX_PACKAGES, "queued_commands": 0,
            "replay_bytes": MAX_MC2_CAMPAIGN_REPLAY_SAVE_BYTES},
    })
}

/// Decimal strings round-trip the authoritative u64 identities without losing
/// precision in JSON clients. Numeric JSON identities are deliberately refused.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(try_from = "String")]
struct Decimal(u64);

impl TryFrom<String> for Decimal {
    type Error = &'static str;
    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("expected an unsigned decimal string");
        }
        value.parse().map(Self).map_err(|_| "decimal exceeds u64")
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeGuardInput {
    runtime_id: Decimal,
    generation: Decimal,
}

impl From<RuntimeGuardInput> for RuntimeGuard {
    fn from(value: RuntimeGuardInput) -> Self {
        Self {
            runtime_id: value.runtime_id.0,
            generation: value.generation.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionGuardInput {
    session_id: Decimal,
    generation: Decimal,
    expected_tick: Decimal,
}

impl From<SessionGuardInput> for SessionGuard {
    fn from(value: SessionGuardInput) -> Self {
        Self {
            session_id: value.session_id.0,
            generation: value.generation.0,
            expected_tick: value.expected_tick.0,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageInput {
    package: PathBuf,
    owner_cache: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    contract: String,
    id: String,
    command: Value,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    Discover,
    Identity,
    /// Bind the installed semantic flow and enter Loading. Opening its pending
    /// level is separate, just as it is for the native gameplay consumer.
    Open {
        guard: RuntimeGuardInput,
        packages: Option<Vec<PackageInput>>,
    },
    OpenPending {
        guard: RuntimeGuardInput,
    },
    Snapshot {
        session_guard: SessionGuardInput,
        cursor: Option<String>,
        limit: Option<usize>,
    },
    Actors {
        session_guard: SessionGuardInput,
        offset: Option<u32>,
        limit: Option<usize>,
    },
    Describe {
        cursor: Option<String>,
        limit: Option<usize>,
    },
    Events {
        session_guard: SessionGuardInput,
        cursor: Decimal,
        limit: usize,
    },
    Step {
        guard: RuntimeGuardInput,
        session_guard: SessionGuardInput,
        input: GameCommand,
    },
    Pause {
        guard: RuntimeGuardInput,
    },
    Resume {
        guard: RuntimeGuardInput,
    },
    PausedFrame {
        guard: RuntimeGuardInput,
        session_guard: SessionGuardInput,
    },
    Completion {
        session_guard: SessionGuardInput,
    },
    AcceptTransition {
        guard: RuntimeGuardInput,
        session_guard: SessionGuardInput,
    },
    CampaignMenu {
        guard: RuntimeGuardInput,
        input: CampaignMenuCommand,
    },
    Save {
        guard: RuntimeGuardInput,
        session_guard: SessionGuardInput,
        path: PathBuf,
    },
    Load {
        guard: RuntimeGuardInput,
        packages: Option<Vec<PackageInput>>,
        path: PathBuf,
    },
    Close {
        guard: RuntimeGuardInput,
        session_guard: SessionGuardInput,
    },
    Shutdown {
        guard: RuntimeGuardInput,
    },
}

#[derive(Debug)]
struct StreamError {
    code: &'static str,
    message: String,
    details: Option<Value>,
}
type Result<T> = std::result::Result<T, StreamError>;

impl StreamError {
    fn new(code: &'static str, message: impl ToString) -> Self {
        // Error messages may originate in an input path or a rejected JSON
        // string. Keep the error response bounded as well as successful pages.
        Self {
            code,
            message: message.to_string().chars().take(2048).collect(),
            details: None,
        }
    }
}

impl From<RuntimeTransitionError> for StreamError {
    fn from(error: RuntimeTransitionError) -> Self {
        let code = match &error {
            RuntimeTransitionError::Guard(error) => error.code.as_str(),
            RuntimeTransitionError::SessionGuard(error) => error.code.as_str(),
            RuntimeTransitionError::InvalidRuntimeId => "INVALID_RUNTIME_ID",
            RuntimeTransitionError::InvalidMode { .. } => "INVALID_RUNTIME_MODE",
            RuntimeTransitionError::InvalidRuntimeFlow { .. } => "INVALID_RUNTIME_FLOW",
            RuntimeTransitionError::InvalidNextDefinition { .. } => "INVALID_NEXT_DEFINITION",
            RuntimeTransitionError::InvalidReplay { .. } => "INVALID_REPLAY",
            RuntimeTransitionError::CompletionNotReady => "COMPLETION_NOT_READY",
            RuntimeTransitionError::InvalidCompletion { .. } => "INVALID_COMPLETION",
            RuntimeTransitionError::GenerationExhausted => "GENERATION_EXHAUSTED",
        };
        Self::new(code, error)
    }
}

impl From<GameSessionError<CampaignBackendError>> for StreamError {
    fn from(error: GameSessionError<CampaignBackendError>) -> Self {
        let details = match &error {
            GameSessionError::Backend(CampaignBackendError::CursorBeyondHead {
                head,
                requested,
            }) => Some(
                json!({"event_head": head.to_string(), "requested_cursor": requested.to_string()}),
            ),
            GameSessionError::Backend(CampaignBackendError::CursorExpired {
                first_available,
                requested,
            }) => Some(
                json!({"first_available_cursor": first_available.to_string(), "requested_cursor": requested.to_string()}),
            ),
            _ => None,
        };
        let code = match &error {
            GameSessionError::Guard(error) => error.code.as_str(),
            GameSessionError::Backend(error) => match error {
                CampaignBackendError::CursorBeyondHead { .. } => "CURSOR_BEYOND_HEAD",
                CampaignBackendError::CursorExpired { .. } => "CURSOR_EXPIRED",
                CampaignBackendError::StepRejected { .. } => "STEP_REJECTED",
                CampaignBackendError::Definition(error) => error.code.as_str(),
                _ => "CAMPAIGN_BACKEND_ERROR",
            },
        };
        let mut error = Self::new(code, error);
        error.details = details;
        error
    }
}

impl From<Mc2CampaignReplaySaveError> for StreamError {
    fn from(error: Mc2CampaignReplaySaveError) -> Self {
        match error {
            Mc2CampaignReplaySaveError::Runtime(error) => error.into(),
            Mc2CampaignReplaySaveError::Definition(error) => Self::new(error.code.as_str(), error),
            error => Self::new("REPLAY_ERROR", error),
        }
    }
}

struct Stream {
    runtime: GameRuntime<Mc2CampaignBackend>,
    resolver: Option<Mc2RuntimePackageResolver>,
}

impl Stream {
    fn new() -> Self {
        Self {
            runtime: GameRuntime::title(1).expect("nonzero local runtime identity"),
            resolver: None,
        }
    }

    fn resolver(&self) -> Result<&Mc2RuntimePackageResolver> {
        self.resolver.as_ref().ok_or_else(|| {
            StreamError::new(
                "MISSING_PACKAGE_SET",
                "open with packages before using campaign operations",
            )
        })
    }

    fn session(&self, guard: SessionGuardInput) -> Result<&GameSession<Mc2CampaignBackend>> {
        let session = self.runtime.session().ok_or_else(|| {
            StreamError::new(
                "INVALID_RUNTIME_MODE",
                "operation requires Playing or Paused",
            )
        })?;
        session
            .completion_status(guard.into())
            .map_err(|error| StreamError::new(error.code.as_str(), error))?;
        Ok(session)
    }

    fn validate_runtime(&self, guard: RuntimeGuardInput) -> Result<()> {
        let expected = self.runtime.identity();
        if guard.runtime_id.0 != expected.runtime_id {
            return Err(StreamError::new(
                "RUNTIME_MISMATCH",
                "runtime_id does not identify this process-owned instance",
            ));
        }
        if guard.generation.0 != expected.generation {
            return Err(StreamError::new(
                "GENERATION_MISMATCH",
                "runtime generation is stale; read identity",
            ));
        }
        Ok(())
    }

    fn require_title(&self, guard: RuntimeGuardInput) -> Result<()> {
        self.validate_runtime(guard)?;
        if self.runtime.identity().mode != RuntimeMode::Title {
            return Err(StreamError::new(
                "INVALID_RUNTIME_MODE",
                "open/load requires Title; close active gameplay first",
            ));
        }
        Ok(())
    }

    fn load_packages(packages: Vec<PackageInput>) -> Result<Mc2RuntimePackageResolver> {
        if packages.is_empty() || packages.len() > MAX_PACKAGES {
            return Err(StreamError::new(
                "INVALID_PACKAGE_COUNT",
                "packages must contain 1..64 installed package bindings",
            ));
        }
        Mc2RuntimePackageResolver::load(packages.into_iter().map(|input| Mc2CampaignInputs {
            package_root: input.package,
            owner_cache: input.owner_cache,
            dynamic_lighting_enabled: true,
            authored_workspace: None,
        }))
        .map_err(|error| StreamError::new(error.code.as_str(), error))
    }

    fn dispatch(&mut self, command: Command) -> Result<Value> {
        match command {
            Command::Discover => Ok(json!({
                "transport": capabilities(), "runtime_contract": GAME_RUNTIME_CONTRACT,
                "session_contract": GAME_SESSION_CONTRACT,
                "flow": self.resolver.as_ref().map(Mc2RuntimePackageResolver::flow),
                "identity": self.runtime.identity(),
                "wire": {"request": {"contract": CONTRACT, "id": "client request ID (1..128 bytes)",
                    "command": {"operation": "identity"}},
                    "runtime_guard": {"runtime_id": "identity.runtime_id", "generation": "identity.generation"},
                    "session_guard": {"session_id": "identity.session.session_id", "generation": "identity.session.generation", "expected_tick": "identity.session.tick"},
                    "integer_identity_encoding": "unsigned decimal strings"},
                "operations": {
                    "discover": {}, "identity": {},
                    "open": {"guard": "runtime_guard", "packages": [{"package": "installed package path", "owner_cache": "optional owner cache path"}],
                        "result": "Loading; packages required initially, omit after close to reuse exact bindings"},
                    "open_pending": {"guard": "runtime_guard", "result": "Playing; resolver selects the pending semantic level"},
                    "snapshot": {"session_guard": "session_guard", "limit": "1..100, default 100", "cursor": "optional returned next_cursor",
                        "result": "authoritative player pose/motion/vitals, objectives, acquired spell IDs/tiers/costs/readiness and paged presentation entities"},
                    "actors": {"session_guard": "session_guard", "limit": "1..100, default 100",
                        "offset": "start at 0; continue with value.next_offset and the same session guard",
                        "kinds": Mc2CampaignBackend::actor_page_kinds().collect::<Vec<_>>(),
                        "result": "guarded page of current semantic actor lifetimes"},
                    "describe": {"limit": "1..100, default 100", "cursor": "optional returned next_cursor", "result": "semantic catalog descriptors; effective values when a session is bound"},
                    "events": {"session_guard": "session_guard", "cursor": "decimal string, initially 0", "limit": "1..100"},
                    "step": {"guard": "runtime_guard", "session_guard": "session_guard", "input": GameCommand::default(), "ticks": 1},
                    "pause": {"guard": "runtime_guard"}, "resume": {"guard": "runtime_guard"},
                    "paused_frame": {"guard": "runtime_guard", "session_guard": "session_guard"},
                    "completion": {"session_guard": "session_guard"},
                    "accept_transition": {"guard": "runtime_guard", "session_guard": "session_guard"},
                    "campaign_menu": {"guard": "runtime_guard", "input": {"action": "dismiss_results | select_mission | start_mission | return_title", "level_id": "select_mission only: semantic ID from campaign_menu.available_levels"}},
                    "save": {"guard": "runtime_guard", "session_guard": "session_guard", "path": "new replay file; existing files refused"},
                    "load": {"guard": "runtime_guard", "path": "existing replay file", "packages": "required only in a fresh process; same schema as open"},
                    "close": {"guard": "runtime_guard", "session_guard": "session_guard", "result": "Title; drops active gameplay and retains immutable package resolver"},
                    "shutdown": {"guard": "runtime_guard", "result": "drops runtime/resolver and exits; EOF also releases ownership"}
                },
                "game_command": {
                    "longitudinal": ["coast", "accelerate", "decelerate"],
                    "lateral": ["neutral", "negative", "positive"],
                    "roll_axis": "i8", "pitch_axis": "i8", "look_yaw": "i16", "look_pitch": "i16",
                    "cast_left": "bool", "cast_right": "bool", "cast_selected": "bool",
                    "recenter": "bool", "full_stop": "bool",
                    "select_spell": "optional acquired ID from snapshot.observation.value.player_spells; validated by the authoritative backend",
                    "omitted_fields": "GameCommand defaults", "unknown_fields": "rejected"
                }
            })),
            Command::Identity => Ok(json!(self.runtime.identity())),
            Command::Open { guard, packages } => {
                self.require_title(guard)?;
                if let Some(packages) = packages {
                    if self.resolver.is_some() {
                        return Err(StreamError::new(
                            "PACKAGE_SET_ALREADY_INSTALLED",
                            "omit packages to reuse the immutable installed package set",
                        ));
                    }
                    let resolver = Self::load_packages(packages)?;
                    resolver.new_game(&mut self.runtime, guard.into())?;
                    self.resolver = Some(resolver);
                } else {
                    let resolver = self.resolver.as_ref().ok_or_else(|| {
                        StreamError::new("MISSING_PACKAGE_SET", "initial open requires packages")
                    })?;
                    resolver.new_game(&mut self.runtime, guard.into())?;
                }
                Ok(json!(self.runtime.identity()))
            }
            Command::OpenPending { guard } => {
                self.validate_runtime(guard)?;
                let resolver = self.resolver.as_ref().ok_or_else(|| {
                    StreamError::new(
                        "MISSING_PACKAGE_SET",
                        "open requires an installed package set",
                    )
                })?;
                resolver
                    .open_pending(&mut self.runtime, guard.into(), 1)
                    .map_err(|error| match error {
                        OpenLevelError::Transition(error) => error.into(),
                        error => StreamError::new("OPEN_LEVEL_FAILED", error),
                    })?;
                Ok(json!(self.runtime.identity()))
            }
            Command::Snapshot {
                session_guard,
                cursor,
                limit,
            } => {
                let limit = page_limit(limit)?;
                let observation = self.session(session_guard)?.snapshot();
                snapshot_page(observation, cursor.as_deref(), limit)
            }
            Command::Actors {
                session_guard,
                offset,
                limit,
            } => {
                let limit = page_limit(limit)?;
                let request =
                    ActorPageRequest::new(offset.unwrap_or(0), limit as u32).ok_or_else(|| {
                        StreamError::new(
                            "INVALID_CURSOR",
                            "actor page offset exceeds the bounded range; start a new page",
                        )
                    })?;
                let page = self
                    .session(session_guard)?
                    .actors(session_guard.into(), request)
                    .map_err(|error| StreamError::new(error.code.as_str(), error))?;
                Ok(json!(page))
            }
            Command::Describe { cursor, limit } => {
                let limit = page_limit(limit)?;
                let catalog = default_catalog()
                    .map_err(|error| StreamError::new(error.code.as_str(), error))?;
                let definition = self.runtime.session().map(GameSession::definition);
                let binding =
                    definition.map_or("unbound", |definition| definition.definition_hash());
                let prefix = format!("catalog:{}:{binding}:", catalog.catalog_hash());
                let offset = page_offset(cursor.as_deref(), &prefix)?;
                let total = catalog.descriptors().len();
                let end = page_end(offset, limit, total)?;
                let descriptors = catalog
                    .descriptors()
                    .skip(offset)
                    .take(end - offset)
                    .map(|descriptor| {
                        let value =
                            definition.and_then(|definition| definition.value(&descriptor.id));
                        descriptor.to_json(value)
                    })
                    .collect::<Vec<_>>();
                Ok(
                    json!({"catalog_hash": catalog.catalog_hash(), "descriptors": descriptors,
                    "page": page_metadata(&prefix, end, limit, total)}),
                )
            }
            Command::Events {
                session_guard,
                cursor,
                limit,
            } => {
                page_limit(Some(limit))?;
                Ok(json!(self.session(session_guard)?.events_after(
                    session_guard.into(),
                    cursor.0,
                    limit
                )?))
            }
            Command::Step {
                guard,
                session_guard,
                input,
            } => {
                self.validate_runtime(guard)?;
                let session = self.runtime.session_mut().ok_or_else(|| {
                    StreamError::new("INVALID_RUNTIME_MODE", "step requires Playing")
                })?;
                let observation = session.step(session_guard.into(), input)?;
                snapshot_page(observation, None, MAX_PAGE)
            }
            Command::Pause { guard } => Ok(json!(self.runtime.pause(guard.into())?)),
            Command::Resume { guard } => Ok(json!(self.runtime.resume(guard.into())?)),
            Command::PausedFrame {
                guard,
                session_guard,
            } => {
                self.runtime
                    .paused_frame(guard.into(), session_guard.into())
                    .map_err(|error| match error {
                        PausedFrameError::Transition(error) => StreamError::from(error),
                        PausedFrameError::Session(error) => StreamError::from(error),
                    })?;
                let observation = self
                    .runtime
                    .session()
                    .expect("successful paused frame retains session")
                    .snapshot();
                snapshot_page(observation, None, MAX_PAGE)
            }
            Command::Completion { session_guard } => Ok(json!(
                self.session(session_guard)?
                    .completion_status(session_guard.into())
                    .map_err(|error| StreamError::new(error.code.as_str(), error))?
            )),
            Command::AcceptTransition {
                guard,
                session_guard,
            } => {
                self.validate_runtime(guard)?;
                let resolver = self.resolver.as_ref().ok_or_else(|| {
                    StreamError::new("MISSING_PACKAGE_SET", "no installed resolver")
                })?;
                Ok(json!(resolver.accept_transition(
                    &mut self.runtime,
                    guard.into(),
                    session_guard.into()
                )?))
            }
            Command::CampaignMenu { guard, input } => {
                self.validate_runtime(guard)?;
                let resolver = self.resolver.as_ref().ok_or_else(|| {
                    StreamError::new("MISSING_PACKAGE_SET", "no installed resolver")
                })?;
                Ok(json!(resolver.campaign_menu_command(
                    &mut self.runtime,
                    guard.into(),
                    input
                )?))
            }
            Command::Save {
                guard,
                session_guard,
                path,
            } => {
                self.validate_runtime(guard)?;
                self.session(session_guard)?;
                let bytes = self.resolver()?.save_replay(&self.runtime)?;
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .map_err(|error| StreamError::new("SAVE_IO_ERROR", error))?;
                file.write_all(&bytes)
                    .and_then(|()| file.sync_all())
                    .map_err(|error| StreamError::new("SAVE_IO_ERROR", error))?;
                Ok(
                    json!({"path": path, "bytes": bytes.len(), "persistence": "authoritative_operation_replay"}),
                )
            }
            Command::Load {
                guard,
                packages,
                path,
            } => {
                self.require_title(guard)?;
                if packages.is_some() && self.resolver.is_some() {
                    return Err(StreamError::new(
                        "PACKAGE_SET_ALREADY_INSTALLED",
                        "omit packages to reuse the immutable installed package set",
                    ));
                }
                let mut bytes = Vec::new();
                File::open(path)
                    .and_then(|file| {
                        file.take((MAX_MC2_CAMPAIGN_REPLAY_SAVE_BYTES + 1) as u64)
                            .read_to_end(&mut bytes)
                    })
                    .map_err(|error| StreamError::new("LOAD_IO_ERROR", error))?;
                if bytes.len() > MAX_MC2_CAMPAIGN_REPLAY_SAVE_BYTES {
                    return Err(StreamError::new(
                        "REPLAY_TOO_LARGE",
                        "replay exceeds maximum save size",
                    ));
                }
                if let Some(packages) = packages {
                    let resolver = Self::load_packages(packages)?;
                    resolver.load_replay(&mut self.runtime, guard.into(), 1, &bytes)?;
                    self.resolver = Some(resolver);
                } else {
                    let resolver = self.resolver.as_ref().ok_or_else(|| {
                        StreamError::new("MISSING_PACKAGE_SET", "initial load requires packages")
                    })?;
                    resolver.load_replay(&mut self.runtime, guard.into(), 1, &bytes)?;
                }
                Ok(json!(self.runtime.identity()))
            }
            Command::Close {
                guard,
                session_guard,
            } => Ok(json!(
                self.runtime.close(guard.into(), session_guard.into())?
            )),
            Command::Shutdown { guard } => {
                self.validate_runtime(guard)?;
                Ok(json!({"shutdown": true}))
            }
        }
    }

    fn response(&mut self, bytes: &[u8], sequence: u64) -> (Value, bool) {
        let request = serde_json::from_slice::<Request>(bytes);
        let (id, result, shutdown) = match request {
            Ok(request) if request.id.is_empty() || request.id.len() > 128 => (
                None,
                Err(StreamError::new(
                    "INVALID_REQUEST_ID",
                    "id must contain 1..128 bytes",
                )),
                false,
            ),
            Ok(request) if request.contract != CONTRACT => (
                Some(request.id),
                Err(StreamError::new("UNSUPPORTED_CONTRACT", CONTRACT)),
                false,
            ),
            Ok(request) => {
                let (result, shutdown) = match serde_json::from_value::<Command>(request.command) {
                    Ok(command) => {
                        let shutdown = matches!(command, Command::Shutdown { .. });
                        (self.dispatch(command), shutdown)
                    }
                    Err(error) => (Err(StreamError::new("MALFORMED_REQUEST", error)), false),
                };
                let shutdown = shutdown && result.is_ok();
                (Some(request.id), result, shutdown)
            }
            Err(error) => (
                None,
                Err(StreamError::new("MALFORMED_REQUEST", error)),
                false,
            ),
        };
        (self.envelope(id, sequence, result), shutdown)
    }

    fn envelope(&self, id: Option<String>, sequence: u64, result: Result<Value>) -> Value {
        let mut response = json!({"contract": CONTRACT, "id": id, "sequence": sequence.to_string(), "identity": self.runtime.identity(), "ok": result.is_ok()});
        match result {
            Ok(data) => response["data"] = data,
            Err(error) => {
                response["error"] =
                    json!({"code": error.code, "message": error.message, "details": error.details})
            }
        }
        response
    }
}

fn page_limit(limit: Option<usize>) -> Result<usize> {
    let limit = limit.unwrap_or(MAX_PAGE);
    if (1..=MAX_PAGE).contains(&limit) {
        Ok(limit)
    } else {
        Err(StreamError::new(
            "INVALID_PAGE_LIMIT",
            "limit must be 1..100",
        ))
    }
}

fn page_offset(cursor: Option<&str>, prefix: &str) -> Result<usize> {
    cursor.map_or(Ok(0), |cursor| {
        cursor
            .strip_prefix(prefix)
            .and_then(|offset| offset.parse().ok())
            .ok_or_else(|| {
                StreamError::new(
                    "INVALID_CURSOR",
                    "cursor belongs to another binding or observation; start a new page",
                )
            })
    })
}

fn page_end(offset: usize, limit: usize, total: usize) -> Result<usize> {
    if offset > total {
        return Err(StreamError::new(
            "INVALID_CURSOR",
            "cursor offset exceeds result count",
        ));
    }
    Ok(offset.saturating_add(limit).min(total))
}

fn page_metadata(prefix: &str, end: usize, limit: usize, total: usize) -> Value {
    json!({"total": total, "limit": limit, "next_cursor": (end < total).then(|| format!("{prefix}{end}")), "complete": end == total})
}

fn snapshot_page(
    mut observation: SessionObservation<CampaignSnapshot>,
    cursor: Option<&str>,
    limit: usize,
) -> Result<Value> {
    let identity = &observation.identity;
    let prefix = format!(
        "snapshot:{}:{}:{}:{:016x}:",
        identity.session_id, identity.generation, identity.tick, observation.value.state_hash
    );
    let offset = page_offset(cursor, &prefix)?;
    let total = observation.value.presentation_entities.len();
    let end = page_end(offset, limit, total)?;
    observation.value.presentation_entities = observation
        .value
        .presentation_entities
        .drain(offset..end)
        .collect();
    Ok(json!({"observation": observation, "page": page_metadata(&prefix, end, limit, total)}))
}

/// Read through a newline even after exceeding the bound, retaining no more
/// than MAX_REQUEST_BYTES. A bad frame cannot consume the following request.
fn read_frame(reader: &mut impl BufRead) -> io::Result<Option<std::result::Result<Vec<u8>, ()>>> {
    let mut frame = Vec::new();
    let mut oversized = false;
    loop {
        let chunk = reader.fill_buf()?;
        if chunk.is_empty() {
            return Ok(if oversized {
                Some(Err(()))
            } else if frame.is_empty() {
                None
            } else {
                Some(Ok(frame))
            });
        }
        let newline = chunk.iter().position(|byte| *byte == b'\n');
        let count = newline.map_or(chunk.len(), |index| index + 1);
        if frame.len().saturating_add(count) > MAX_REQUEST_BYTES {
            oversized = true;
        }
        if !oversized {
            frame.extend_from_slice(&chunk[..count]);
        }
        reader.consume(count);
        if newline.is_some() {
            return Ok(Some(if oversized { Err(()) } else { Ok(frame) }));
        }
    }
}

fn serve(reader: &mut impl BufRead, writer: &mut impl Write) -> io::Result<()> {
    let mut stream = Stream::new();
    let mut sequence = 0_u64;
    while let Some(frame) = read_frame(reader)? {
        sequence = sequence
            .checked_add(1)
            .ok_or_else(|| io::Error::other("response sequence exhausted"))?;
        let (response, shutdown) = match frame {
            Ok(bytes) => stream.response(&bytes, sequence),
            Err(()) => (
                stream.envelope(
                    None,
                    sequence,
                    Err(StreamError::new(
                        "REQUEST_TOO_LARGE",
                        "request exceeds 65536 bytes including newline",
                    )),
                ),
                false,
            ),
        };
        let mut bytes = serde_json::to_vec(&response)?;
        if bytes.len() + 1 > MAX_RESPONSE_BYTES {
            bytes = serde_json::to_vec(
                &json!({"contract": CONTRACT, "id": response["id"], "sequence": sequence.to_string(), "ok": false,
                "error": {"code": "RESPONSE_TOO_LARGE", "message": "response exceeds transport bound; inspect identity and use smaller pages"}}),
            )?;
        }
        writer.write_all(&bytes)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        if shutdown {
            break;
        }
    }
    // Dropping stream releases the sole runtime and resolver on shutdown,
    // EOF, a broken pipe, or any other transport failure.
    Ok(())
}

pub(crate) fn run() -> ExitCode {
    match serve(&mut io::stdin().lock(), &mut io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("command stream I/O failed: {error}");
            ExitCode::from(3)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_stream_recovers_frames_and_rejects_requests_without_mutation() {
        let mut input = vec![b'x'; MAX_REQUEST_BYTES + 8];
        input.push(b'\n');
        for command in [
            json!({"operation": "identity"}),
            json!({"operation": "pause", "guard": {"runtime_id": "1", "generation": "0"}}),
            json!({"operation": "step", "guard": {"runtime_id": "1", "generation": "1"}, "session_guard": {"session_id": "1", "generation": "1", "expected_tick": "0"}, "input": {"unknown": true}}),
            json!({"operation": "events", "session_guard": {"session_id": "1", "generation": "1", "expected_tick": "0"}, "cursor": "0", "limit": 101}),
            json!({"operation": "shutdown", "guard": {"runtime_id": "1", "generation": "1"}}),
            json!({"operation": "identity"}),
        ] {
            input.extend(
                serde_json::to_vec(
                    &json!({"contract": CONTRACT, "id": "test", "command": command}),
                )
                .unwrap(),
            );
            input.push(b'\n');
        }
        let mut output = Vec::new();
        serve(&mut io::Cursor::new(input), &mut output).unwrap();
        let responses = output
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice::<Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            responses.len(),
            6,
            "shutdown stops before the next queued frame"
        );
        for (index, response) in responses.iter().enumerate() {
            assert_eq!(response["sequence"], (index + 1).to_string());
            assert_eq!(response["identity"], responses[1]["identity"]);
        }
        assert_eq!(responses[0]["error"]["code"], "REQUEST_TOO_LARGE");
        assert_eq!(responses[2]["error"]["code"], "GENERATION_MISMATCH");
        assert_eq!(responses[3]["error"]["code"], "MALFORMED_REQUEST");
        assert_eq!(
            responses[3]["id"], "test",
            "valid envelope IDs survive command rejection"
        );
        assert_eq!(responses[4]["error"]["code"], "INVALID_PAGE_LIMIT");
        assert_eq!(responses[5]["data"]["shutdown"], true);
    }
}
