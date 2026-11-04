//! Rust-owned title/loading/playing runtime transport.
//!
//! A runtime remains behind one handle for its entire lifetime. Once it
//! reaches `playing`, the existing `mc_game_session_*_v1` exports operate on
//! the nested `GameSession`; this module does not create a second gameplay
//! authority or a parallel step API.

use super::secret_transition_abi::BoundaryTransitionReceipt;
use super::*;
use serde::Deserialize;
use sim_core::{
    campaign::backend::{
        MAX_MC2_CAMPAIGN_REPLAY_SAVE_BYTES, MC2_CAMPAIGN_REPLAY_SAVE_CONTRACT, Mc2CampaignBackend,
        Mc2CampaignInputs, Mc2CampaignReplaySaveError, Mc2RuntimePackageResolver,
    },
    game_runtime::{
        COMPLETION_TRANSITION_RECEIPT, CompletionTransitionReceipt, GameRuntime, OpenLevelError,
        PausedFrameError, RuntimeGuard, RuntimeGuardError, RuntimeGuardErrorCode, RuntimeMode,
        RuntimeTransitionError,
    },
};
use sim_session::{GameSession, OpenSessionError, SessionGuard};

const GAME_RUNTIME_TRANSPORT_CONTRACT: &str = "carpet-flight-native-game-runtime-json/v12";
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
    presentation_cache_path: Option<String>,
}

struct InstalledCampaignInput {
    campaign: Mc2CampaignInputs,
    presentation_cache: Option<std::path::PathBuf>,
    sound_bank: Option<super::campaign_sound_bank::CampaignSoundBank>,
}

fn preflight_sound_banks(
    inputs: &mut [InstalledCampaignInput],
    resolver: &Mc2RuntimePackageResolver,
) -> AbiResult<()> {
    // Validate every candidate before replacing any retained binding. A bad
    // destination must leave the current session and voice bank intact.
    let banks = inputs
        .iter()
        .map(|input| {
            let selection = sim_core::campaign::spec::CampaignDefinition::declared_selection(
                &input.campaign.package_root,
            )
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))?;
            let (owner_hash, campaign_hash) = resolver
                .installed_audio_identity(selection)
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "sound bank has no exact installed source identity",
                    )
                })?;
            for root in input
                .campaign
                .owner_cache
                .iter()
                .chain(input.presentation_cache.iter())
            {
                if let Some(bank) = super::campaign_sound_bank::CampaignSoundBank::load_if_present(
                    root,
                    selection,
                    owner_hash,
                    campaign_hash,
                )? {
                    return Ok(Some(bank));
                }
            }
            Ok(None)
        })
        .collect::<AbiResult<Vec<_>>>()?;
    for (input, bank) in inputs.iter_mut().zip(banks) {
        input.sound_bank = bank;
    }
    Ok(())
}

pub(super) struct RuntimeRecord {
    runtime: GameRuntime<Mc2CampaignBackend>,
    resolver: Option<Mc2RuntimePackageResolver>,
    installed_inputs: Vec<InstalledCampaignInput>,
    campaign_presentation: Option<super::campaign_abi::CampaignSessionRecord>,
    // A presentation read borrows a snapshot. Rebuild it after authoritative
    // mutation, not once per frame-plane, palette, entity and HUD query.
    campaign_presentation_dirty: bool,
    audio: super::campaign_audio_state::CampaignRuntimeAudio,
    completion_transition_receipt: Option<BoundaryTransitionReceipt>,
    closed: bool,
}

struct CampaignPreferenceRollback {
    speech_enabled: bool,
    show_foreign_text: bool,
    operation_count: usize,
    dynamic_lighting_enabled: bool,
    lighting_change_count: usize,
}

impl RuntimeRecord {
    fn title(runtime_id: u64) -> AbiResult<Self> {
        let runtime = GameRuntime::title(runtime_id).map_err(map_transition_error)?;
        Ok(Self {
            runtime,
            resolver: None,
            installed_inputs: Vec::new(),
            campaign_presentation: None,
            campaign_presentation_dirty: true,
            audio: super::campaign_audio_state::CampaignRuntimeAudio::default(),
            completion_transition_receipt: None,
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

    pub(super) fn campaign_snapshot(
        &self,
    ) -> AbiResult<sim_session::SessionObservation<sim_session::CampaignSnapshot>> {
        let session = self.session()?;
        let mut observation = session.snapshot();
        let local_slot = session
            .backend()
            .campaign_presentation_source()
            .simulation()
            .shared_spells()
            .local_player()
            .slot;
        if let Some(local) = observation
            .value
            .source_scoreboard
            .players
            .iter_mut()
            .find(|player| player.source_slot == local_slot)
        {
            local.display_name = self
                .runtime
                .identity()
                .player_preferences
                .preferences
                .player_name;
        }
        Ok(observation)
    }

    pub(super) fn session_mut(&mut self) -> AbiResult<&mut GameSession<Mc2CampaignBackend>> {
        self.ensure_open()?;
        self.campaign_presentation_dirty = true;
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

    pub(super) fn step_with_preferences(
        &mut self,
        guard: SessionGuard,
        command: sim_session::GameCommand,
    ) -> AbiResult<()> {
        self.ensure_open()?;
        if self.runtime.identity().mode != RuntimeMode::Playing {
            self.runtime.reset_preference_input();
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "INVALID_RUNTIME_MODE: preferences step requires playing",
            ));
        }
        let previous = self.bind_campaign_preferences(guard)?;
        let result = self
            .runtime
            .advance_player_with_preferences(guard, command)
            .map_err(|error| match error {
                sim_core::game_runtime::preferences::PreferenceStepError::Unavailable => {
                    AbiError::new(
                        MC_STATUS_INVALID_ARGUMENT,
                        "INVALID_RUNTIME_MODE: preferences step requires playing",
                    )
                }
                sim_core::game_runtime::preferences::PreferenceStepError::Session(error) => {
                    super::game_session_abi::map_campaign_session_error(error)
                }
            });
        if let Err(error) = result {
            self.rollback_campaign_preferences(previous);
            return Err(error);
        }
        self.sync_audio(false)
    }

    fn bind_campaign_preferences(
        &mut self,
        guard: SessionGuard,
    ) -> AbiResult<CampaignPreferenceRollback> {
        self.session()?.completion_status(guard).map_err(|error| {
            super::game_session_abi::map_campaign_session_error(
                sim_session::GameSessionError::Guard(error),
            )
        })?;
        let preferences = self.runtime.identity().player_preferences.preferences;
        let show_foreign_text = self.runtime.show_foreign_objective_text();
        let backend = self.session_mut()?.backend_mut();
        let previous = CampaignPreferenceRollback {
            speech_enabled: backend.speech_enabled(),
            show_foreign_text: backend.show_foreign_objective_text(),
            operation_count: backend.replay_operation_count(),
            dynamic_lighting_enabled: backend.dynamic_lighting_enabled(),
            lighting_change_count: backend.dynamic_lighting_change_count(),
        };
        backend
            .set_objective_presentation_preferences(preferences.speech_enabled, show_foreign_text);
        backend.set_dynamic_lighting_enabled(preferences.dynamic_lighting);
        Ok(previous)
    }

    fn rollback_campaign_preferences(&mut self, previous: CampaignPreferenceRollback) {
        if let Some(session) = self.runtime.session_mut() {
            session
                .backend_mut()
                .restore_dynamic_lighting_after_rejection(
                    previous.dynamic_lighting_enabled,
                    previous.lighting_change_count,
                );
            session
                .backend_mut()
                .restore_speech_preference_after_rejection(
                    previous.speech_enabled,
                    previous.show_foreign_text,
                    previous.operation_count,
                );
        }
    }

    fn reset_audio(&mut self) -> AbiResult<()> {
        self.campaign_presentation_dirty = true;
        self.audio = super::campaign_audio_state::CampaignRuntimeAudio::default();
        if let Some(session) = self.runtime.session() {
            let campaign = session.backend().campaign_presentation_source();
            if let Some(bank) = self
                .installed_inputs
                .iter()
                .filter_map(|input| input.sound_bank.as_ref())
                .find(|bank| bank.selection() == campaign.definition().identity)
            {
                self.audio.bind_sound_bank(bank.clone())?;
            }
            self.audio.reset(campaign, session.identity().generation);
        }
        Ok(())
    }

    fn sync_audio(&mut self, paused: bool) -> AbiResult<()> {
        let session = self.runtime.session().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "campaign audio needs an active session",
            )
        })?;
        let updates = self.audio.sync(
            session.backend().campaign_presentation_source(),
            session.identity().generation,
            paused,
        )?;
        if let Some(music) = self
            .campaign_presentation
            .as_mut()
            .and_then(|record| record.day_music.as_mut())
        {
            for (mode, multiplier) in updates {
                music.update(mode, multiplier);
            }
        }
        Ok(())
    }

    fn rebind_audio_generation(&mut self) {
        self.campaign_presentation_dirty = true;
        if let Some(session) = self.runtime.session() {
            let generation = session.identity().generation;
            self.audio.generation = generation;
            for command in self.audio.commands.values_mut() {
                command.generation = generation;
            }
            if let Some(record) = self.campaign_presentation.as_mut() {
                record.generation = generation;
                if let super::campaign_abi::CampaignSessionKind::Campaign {
                    day_frame_state, ..
                } = &mut record.kind
                {
                    *day_frame_state = DayIndexedFrameState::new(generation);
                }
            }
        }
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
        let mut inputs = installed_package_set_inputs(request_json, request_json_len)?;
        let resolver =
            Mc2RuntimePackageResolver::load(inputs.iter().map(|input| input.campaign.clone()))
                .map_err(|error| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        format!("cannot load installed game runtime package set: {error}"),
                    )
                })?;
        preflight_sound_banks(&mut inputs, &resolver)?;
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
        self.installed_inputs = inputs;
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
        preflight_sound_banks(&mut self.installed_inputs, resolver)?;
        resolver
            .open_pending(&mut self.runtime, guard, handle)
            .map_err(map_open_level_error)?;
        self.reset_audio()?;
        Ok(())
    }

    fn loading_presentation(
        &self,
        handle: u64,
        generation: u64,
    ) -> AbiResult<sim_core::campaign::backend::Mc2LoadingPresentation> {
        self.validate_transition(handle, generation, RuntimeMode::Loading)?;
        self.resolver
            .as_ref()
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "MISSING_RUNTIME_RESOLVER: loading presentation requires installed packages",
                )
            })?
            .loading_presentation(
                &self.runtime,
                RuntimeGuard {
                    runtime_id: handle,
                    generation,
                },
            )
            .map_err(map_transition_error)
    }

    fn cancel_loading(&mut self, handle: u64, generation: u64) -> AbiResult<()> {
        self.validate_transition(handle, generation, RuntimeMode::Loading)?;
        self.resolver
            .as_ref()
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "MISSING_RUNTIME_RESOLVER: cancel loading requires installed packages",
                )
            })?
            .cancel_loading(
                &mut self.runtime,
                RuntimeGuard {
                    runtime_id: handle,
                    generation,
                },
            )
            .map_err(map_transition_error)?;
        self.completion_transition_receipt = None;
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
        if !matches!(
            mode,
            RuntimeMode::Playing | RuntimeMode::Paused | RuntimeMode::Campaign | RuntimeMode::Title
        ) {
            return Err(map_transition_error(RuntimeTransitionError::InvalidMode {
                expected: RuntimeMode::Playing,
                actual: mode,
            }));
        }
        self.validate_transition(handle, runtime_generation, mode)?;
        if matches!(mode, RuntimeMode::Campaign | RuntimeMode::Title) {
            if session_generation != 0 || expected_tick != 0 || self.runtime.session().is_some() {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "INVALID_CAMPAIGN_SAVE_GUARD: map save requires no session and zero session generation/tick",
                ));
            }
            return self
                .resolver
                .as_ref()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_INVALID_ARGUMENT,
                        "MISSING_RUNTIME_RESOLVER: map save requires installed packages",
                    )
                })?
                .save_replay(&self.runtime)
                .map_err(map_replay_save_error);
        }
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
        self.load_replay_with_source_context(
            handle,
            generation,
            package_set_json,
            package_set_json_len,
            save_json,
            None,
        )
    }

    fn load_replay_with_source_context(
        &mut self,
        handle: u64,
        generation: u64,
        package_set_json: String,
        package_set_json_len: u32,
        save_json: &[u8],
        source_context: Option<sim_core::native_world::GlobalSpecialState>,
    ) -> AbiResult<()> {
        self.validate_transition(handle, generation, RuntimeMode::Title)?;
        if self.resolver.is_some() {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "PACKAGE_SET_ALREADY_INSTALLED: replay load requires a fresh runtime",
            ));
        }
        let mut inputs = installed_package_set_inputs(package_set_json, package_set_json_len)?;
        let resolver =
            Mc2RuntimePackageResolver::load(inputs.iter().map(|input| input.campaign.clone()))
                .map_err(|error| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        format!("cannot load replay package set: {error}"),
                    )
                })?;
        preflight_sound_banks(&mut inputs, &resolver)?;
        let guard = RuntimeGuard {
            runtime_id: handle,
            generation,
        };
        match source_context {
            Some(context) => resolver.load_replay_with_source_context(
                &mut self.runtime,
                guard,
                handle,
                save_json,
                context,
            ),
            None => resolver.load_replay(&mut self.runtime, guard, handle, save_json),
        }
        .map_err(map_replay_save_error)?;
        self.resolver = Some(resolver);
        self.installed_inputs = inputs;
        self.reset_audio()?;
        Ok(())
    }

    pub(super) fn take_campaign_presentation_record(
        &mut self,
    ) -> AbiResult<super::campaign_abi::CampaignSessionRecord> {
        let preferences = &self.runtime.identity().player_preferences.preferences;
        let render_options = sim_core::day_indexed_frame::WorldRenderOptions {
            reflections: preferences.reflections,
            sky: preferences.sky,
            shadows: preferences.shadows,
        };
        let generation = self.session()?.identity().generation;
        if !self.campaign_presentation_dirty
            && self
                .campaign_presentation
                .as_ref()
                .is_some_and(|record| record.generation == generation)
        {
            let mut record = self.campaign_presentation.take().unwrap();
            if let super::campaign_abi::CampaignSessionKind::Campaign {
                day_frame_state, ..
            } = &mut record.kind
            {
                day_frame_state.set_render_options(render_options);
            }
            return Ok(record);
        }
        let (generation, campaign) = {
            let session = self.session()?;
            (
                session.identity().generation,
                session.backend().campaign_presentation_source().clone(),
            )
        };
        let selection = campaign.definition().identity;
        let input = self
            .installed_inputs
            .iter()
            .find(|input| {
                sim_core::campaign::spec::CampaignDefinition::declared_selection(
                    &input.campaign.package_root,
                )
                .is_ok_and(|candidate| candidate == selection)
            })
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "active game-runtime campaign has no selection-bound installed resource",
                )
            })?;
        let inspection = campaign.inspection_arc();
        let retained = self.campaign_presentation.take().and_then(|record| {
            if record.generation != generation {
                return None;
            }
            let super::campaign_abi::CampaignSessionKind::Campaign {
                reset,
                day_frame_assets,
                day_frame_state,
                ..
            } = record.kind
            else {
                return None;
            };
            (reset.definition_identity() == selection).then_some((
                day_frame_assets,
                day_frame_state,
                record.day_music,
                record.effect_state_hashes,
            ))
        });
        let initializing = retained.is_none();
        let (assets, mut state, mut music, mut effect_state_hashes) = match retained {
            Some(retained) => retained,
            None => (
                load_optional_day_frame_assets(
                    input.campaign.owner_cache.as_deref().and_then(Path::to_str),
                    inspection.as_ref(),
                    Some(&campaign.definition().package_sha256),
                )?
                .map(Arc::new),
                DayIndexedFrameState::new(generation),
                super::campaign_audio::load_campaign_music(
                    input
                        .presentation_cache
                        .as_deref()
                        .or(input.campaign.owner_cache.as_deref()),
                    selection,
                    inspection.package_sha256(),
                    &campaign.definition().package_sha256,
                )?,
                BTreeMap::new(),
            ),
        };
        state.set_render_options(render_options);
        if initializing
            && (self.runtime.identity().mode == RuntimeMode::Paused
                || !self
                    .runtime
                    .identity()
                    .player_preferences
                    .preferences
                    .music_enabled)
        {
            if let Some(music) = music.as_mut() {
                music.stop();
            }
        }
        for command in self.audio.commands.values() {
            effect_state_hashes
                .insert(command.event_sequence.saturating_sub(1), command.state_hash);
        }
        while effect_state_hashes.len() > 16_384 {
            effect_state_hashes.pop_first();
        }
        self.campaign_presentation_dirty = false;
        Ok(super::campaign_abi::CampaignSessionRecord {
            kind: super::campaign_abi::CampaignSessionKind::Campaign {
                inspection,
                reset: super::campaign_abi::CampaignAuthority::Descriptor(campaign),
                day_frame_assets: assets,
                day_frame_state: state,
            },
            generation,
            day_music: music,
            effect_state_hashes,
        })
    }

    pub(super) fn retain_campaign_presentation_record(
        &mut self,
        record: super::campaign_abi::CampaignSessionRecord,
    ) {
        self.campaign_presentation = Some(record);
    }

    fn pause(&mut self, handle: u64, generation: u64) -> AbiResult<()> {
        self.validate_transition(handle, generation, RuntimeMode::Playing)?;
        self.runtime
            .pause(RuntimeGuard {
                runtime_id: handle,
                generation,
            })
            .map_err(map_transition_error)?;
        // PauseUnpauseGame_18BB0 ends samples and stops music. CD speech
        // has separate ownership and continues across this edge.
        self.reset_audio()?;
        if let Some(music) = self
            .campaign_presentation
            .as_mut()
            .and_then(|r| r.day_music.as_mut())
        {
            music.stop();
        }
        self.rebind_audio_generation();
        Ok(())
    }

    fn paused_frame(
        &mut self,
        handle: u64,
        runtime_generation: u64,
        session_generation: u64,
        expected_tick: u64,
    ) -> AbiResult<()> {
        self.paused_frame_timed(
            handle,
            runtime_generation,
            session_generation,
            expected_tick,
            0,
            false,
        )
    }

    fn paused_frame_timed(
        &mut self,
        handle: u64,
        runtime_generation: u64,
        session_generation: u64,
        expected_tick: u64,
        elapsed_centiseconds: u32,
        show_objective: bool,
    ) -> AbiResult<()> {
        self.validate_transition(handle, runtime_generation, RuntimeMode::Paused)?;
        self.campaign_presentation_dirty = true;
        let guard = SessionGuard {
            session_id: handle,
            generation: session_generation,
            expected_tick,
        };
        self.runtime
            .paused_campaign_frame_timed(
                RuntimeGuard {
                    runtime_id: handle,
                    generation: runtime_generation,
                },
                guard,
                elapsed_centiseconds,
                show_objective,
            )
            .map_err(map_paused_frame_error)?;
        self.sync_audio(true)?;
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
        if self
            .runtime
            .identity()
            .player_preferences
            .preferences
            .music_enabled
        {
            if let Some(music) = self
                .campaign_presentation
                .as_mut()
                .and_then(|r| r.day_music.as_mut())
            {
                music.restart();
            }
        }
        self.rebind_audio_generation();
        Ok(())
    }

    fn accept_transition(
        &mut self,
        handle: u64,
        runtime_generation: u64,
        session_generation: u64,
        expected_tick: u64,
    ) -> AbiResult<()> {
        self.ensure_open()?;
        let resolver = self.resolver.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "MISSING_RUNTIME_RESOLVER: completion transition requires an installed package set",
            )
        })?;
        let receipt = resolver
            .accept_transition(
                &mut self.runtime,
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
        self.completion_transition_receipt = Some(BoundaryTransitionReceipt::OrdinaryCompletion {
            ordinary_completion: receipt,
        });
        self.campaign_presentation = None;
        Ok(())
    }

    fn completion_transition_receipt(&self) -> AbiResult<&CompletionTransitionReceipt> {
        self.ensure_open()?;
        self.completion_transition_receipt
            .as_ref()
            .and_then(BoundaryTransitionReceipt::ordinary_completion)
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "NO_COMPLETION_TRANSITION_RECEIPT: no completion transition has been accepted",
                )
            })
    }

    fn campaign_menu_command(
        &mut self,
        handle: u64,
        generation: u64,
        request: &str,
    ) -> AbiResult<()> {
        self.validate_transition(handle, generation, RuntimeMode::Campaign)?;
        let command = serde_json::from_str::<sim_core::game_runtime::CampaignMenuCommand>(request)
            .map_err(|error| {
                AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("INVALID_CAMPAIGN_ACTION: {error}"),
                )
            })?;
        let resolver = self.resolver.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "MISSING_RUNTIME_RESOLVER: campaign requires installed packages",
            )
        })?;
        resolver
            .campaign_menu_command(
                &mut self.runtime,
                RuntimeGuard {
                    runtime_id: handle,
                    generation,
                },
                command,
            )
            .map_err(map_transition_error)?;
        self.completion_transition_receipt = None;
        Ok(())
    }

    fn finish_mission_if_ready(
        &mut self,
        handle: u64,
        runtime_generation: u64,
        session_generation: u64,
        expected_tick: u64,
    ) -> AbiResult<()> {
        self.validate_transition(handle, runtime_generation, RuntimeMode::Playing)?;
        let resolver = self.resolver.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "MISSING_RUNTIME_RESOLVER: mission boundary requires installed packages",
            )
        })?;
        let exit = self
            .session()?
            .backend()
            .campaign_presentation_source()
            .simulation()
            .level_exit_sequence();
        if exit.complete && exit.result_code == 0x1a {
            preflight_sound_banks(&mut self.installed_inputs, resolver)?;
        }
        if let Some(receipt) = resolver
            .advance_secret_if_ready(
                &mut self.runtime,
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
            .map_err(map_transition_error)?
        {
            self.completion_transition_receipt =
                Some(BoundaryTransitionReceipt::SecretTransition {
                    secret_transition: receipt,
                });
            self.campaign_presentation = None;
            self.reset_audio()?;
            return Ok(());
        }
        let completion = self
            .session()?
            .completion_status(SessionGuard {
                session_id: handle,
                generation: session_generation,
                expected_tick,
            })
            .map_err(|error| {
                map_transition_error(RuntimeTransitionError::SessionGuard(Box::new(error)))
            })?
            .value;
        match completion {
            sim_session::GameCompletionStatus::InProgress => Ok(()),
            sim_session::GameCompletionStatus::Completed { .. } => self.accept_transition(
                handle,
                runtime_generation,
                session_generation,
                expected_tick,
            ),
            sim_session::GameCompletionStatus::Unsupported => Err(map_transition_error(
                RuntimeTransitionError::InvalidCompletion {
                    message: "gameplay backend does not expose semantic completion".to_owned(),
                },
            )),
            sim_session::GameCompletionStatus::Invalid { message } => Err(map_transition_error(
                RuntimeTransitionError::InvalidCompletion { message },
            )),
        }
    }

    fn close(
        &mut self,
        handle: u64,
        runtime_generation: u64,
        session_generation: u64,
        expected_tick: u64,
    ) -> AbiResult<()> {
        self.ensure_open()?;
        let resolver = self.resolver.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "campaign return needs its installed package resolver",
            )
        })?;
        if let Some(receipt) = resolver
            .abandon_secret_if_active(
                &mut self.runtime,
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
            .map_err(map_transition_error)?
        {
            self.completion_transition_receipt =
                Some(BoundaryTransitionReceipt::SecretTransition {
                    secret_transition: receipt,
                });
            self.campaign_presentation = None;
            self.reset_audio()?;
            return Ok(());
        }
        self.resolver
            .as_ref()
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign return needs its installed package resolver",
                )
            })?
            .close_to_title(
                &mut self.runtime,
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
        self.completion_transition_receipt = None;
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
            runtime.campaign_presentation_dirty = true;
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
) -> AbiResult<Vec<InstalledCampaignInput>> {
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
                || location
                    .presentation_cache_path
                    .as_deref()
                    .is_some_and(|path| !path.is_empty() && path.trim().is_empty())
            {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "installed package resource paths must be nonblank",
                ));
            }
            let campaign = campaign_inputs(
                location.package_dir,
                location.owner_cache_path.unwrap_or_default(),
            )?;
            Ok(InstalledCampaignInput {
                campaign,
                sound_bank: None,
                presentation_cache: location
                    .presentation_cache_path
                    .filter(|path| !path.is_empty())
                    .map(std::path::PathBuf::from),
            })
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
            "transport_version": 12,
            "definition_contract": sim_core::game_definition::GAME_DEFINITION_SCHEMA,
            "semantic_contract_version": sim_core::game_definition::SEMANTIC_CONTRACT_VERSION,
            "encoding": "utf-8-json",
            "u64_counter_encoding": "decimal-string",
            "modes": ["title", "campaign", "loading", "playing", "paused"],
            "transitions": ["bootstrap_package_set", "restart_new_game", "show_new_game_map", "open_level", "cancel_loading", "load_replay_save", "pause", "paused_frame", "resume", "accept_completion_transition", "finish_mission_if_ready", "campaign_menu_command", "close"],
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
                "warm_load_transport": "mc_game_runtime_load_replay_from_runtime_json_v1",
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
            "frontend_media": {
                "contract": sim_core::game_runtime::media::MEDIA_CONTRACT,
                "command_transport": "mc_game_runtime_media_command_json_v1",
                "observation": "identity.frontend_media",
                "maximum_command_json_bytes": 256 * 1024,
                "guards": ["runtime_generation", "title_mode"]
            },
            "player_preferences": {
                "command_transport": "mc_game_runtime_preferences_command_json_v1",
                "observation": "identity.player_preferences",
                "maximum_command_json_bytes": 64 * 1024,
                "guards": ["runtime_generation", "frontend_or_opening_install"]
            },
            "runtime_flow": {
                "title": "absent",
                "loading": "pending_definition.runtime_flow",
                "playing_and_paused": "active_runtime_flow",
                "presentation_cue": "identity.presentation"
            },
            "loading_presentation": {
                "schema": "mc2-loading-presentation/v1",
                "transport": "mc_game_runtime_loading_presentation_json_v1",
                "cancel_transport": "mc_game_runtime_cancel_loading_v1",
                "destination_authority": "exact_installed_pending_definition",
                "guards": ["runtime_generation", "loading_mode"]
            },
            "completion_transition": {
                "receipt_contract": COMPLETION_TRANSITION_RECEIPT,
                "accept_transport": "mc_game_runtime_accept_transition_v1",
                "receipt_transport": "mc_game_runtime_completion_transition_receipt_json_v1",
                "destination_authority": "rust_owned_semantic_flow",
                "package_authority": "installed_package_resolver",
                "presentation_sequence": "ordered_rust_owned_semantic_cues",
                "guards": ["runtime_generation", "session_generation", "expected_tick"]
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
            let mut identity = serde_json::to_value(runtime.runtime.identity())
                .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))?;
            if let Some(speech) = runtime
                .resolver
                .as_ref()
                .and_then(|resolver| resolver.campaign_briefing_speech(&runtime.runtime))
            {
                identity["campaign_menu"]["briefing_speech"] = serde_json::to_value(speech)
                    .map_err(|error| {
                        AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string())
                    })?;
            }
            super::game_session_abi::write_json(&identity, buffer, capacity, out_length)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_audio_command_for_event_v1(
    handle: u64,
    generation: u64,
    event_sequence: u64,
    out_command: *mut McAudioCommandV2,
    out_command_size: u32,
) -> i32 {
    boundary_value(out_command, out_command_size, || {
        with_runtime(handle, |record| {
            record.audio.validate_generation(generation)?;
            record
                .audio
                .commands
                .get(&event_sequence)
                .copied()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!("campaign event {event_sequence} has no retained audio command"),
                    )
                })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_audio_voice_complete_v1(
    handle: u64,
    generation: u64,
    voice_index: u16,
    event_sequence: u64,
) -> i32 {
    boundary(|| {
        with_runtime_mut(handle, |record| {
            record.audio.validate_generation(generation)?;
            super::audio_voice_pool::complete_audio_voice(
                &mut record.audio.voices,
                voice_index,
                event_sequence,
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_audio_command_release_v1(
    handle: u64,
    generation: u64,
    event_sequence: u64,
) -> i32 {
    boundary(|| {
        with_runtime_mut(handle, |record| {
            record.audio.validate_generation(generation)?;
            let command =
                record.audio.commands.get(&event_sequence).ok_or_else(|| {
                    AbiError::new(MC_STATUS_BOUNDS, "campaign audio event is absent")
                })?;
            if command.action == 0 {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign audio event is still pending",
                ));
            }
            record.audio.commands.remove(&event_sequence);
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_audio_command_discard_v1(
    handle: u64,
    generation: u64,
    event_sequence: u64,
) -> i32 {
    boundary(|| {
        with_runtime_mut(handle, |record| {
            record.audio.validate_generation(generation)?;
            let command = record
                .audio
                .commands
                .get(&event_sequence)
                .copied()
                .ok_or_else(|| AbiError::new(MC_STATUS_BOUNDS, "campaign audio event is absent"))?;
            if command.action == 0 {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign audio event is still pending",
                ));
            }
            if let Some(voice) = record
                .audio
                .voices
                .get_mut(usize::from(command.voice_index))
            {
                if voice.is_some_and(|voice| voice.event_sequence == event_sequence) {
                    *voice = None;
                }
            }
            record.audio.commands.remove(&event_sequence);
            Ok(())
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
        load_runtime_replay_input(
            handle,
            generation,
            package_set_json,
            package_set_json_len,
            save_json,
            save_json_len,
            None,
        )
    })
}

/// The source's nonserialized process globals survive a menu Load. Capture
/// them from the guarded old owner without destroying it before verification.
#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_load_replay_from_runtime_json_v1(
    handle: u64,
    generation: u64,
    source_handle: u64,
    source_generation: u64,
    package_set_json: *const u8,
    package_set_json_len: u32,
    save_json: *const u8,
    save_json_len: u32,
) -> i32 {
    boundary(|| {
        load_runtime_replay_input(
            handle,
            generation,
            package_set_json,
            package_set_json_len,
            save_json,
            save_json_len,
            Some((source_handle, source_generation)),
        )
    })
}

fn load_runtime_replay_input(
    handle: u64,
    generation: u64,
    package_set_json: *const u8,
    package_set_json_len: u32,
    save_json: *const u8,
    save_json_len: u32,
    source_runtime: Option<(u64, u64)>,
) -> AbiResult<()> {
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
    if usize::try_from(save_json_len).unwrap_or(usize::MAX) > MAX_MC2_CAMPAIGN_REPLAY_SAVE_BYTES {
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
    let source_context = source_runtime
        .map(|(source_handle, source_generation)| {
            if source_handle == handle {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "warm replay load requires a distinct source runtime",
                ));
            }
            with_runtime(source_handle, |record| {
                record.validate_transition(
                    source_handle,
                    source_generation,
                    record.runtime.identity().mode,
                )?;
                record
                    .resolver
                    .as_ref()
                    .ok_or_else(|| {
                        AbiError::new(
                            MC_STATUS_INVALID_ARGUMENT,
                            "warm replay source has no installed MC2 resolver",
                        )
                    })?
                    .source_global_context(&record.runtime)
                    .map_err(map_transition_error)
            })
        })
        .transpose()?;
    with_runtime_mut(handle, |runtime| {
        runtime.load_replay_with_source_context(
            handle,
            generation,
            package_set_json,
            package_set_json_len,
            save_json.as_bytes(),
            source_context,
        )
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_open_level_v1(handle: u64, generation: u64) -> i32 {
    boundary(|| with_runtime_mut(handle, |runtime| runtime.open_level(handle, generation)))
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_show_new_game_map_v1(handle: u64, generation: u64) -> i32 {
    boundary(|| {
        with_runtime_mut(handle, |record| {
            record
                .resolver
                .as_ref()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_INVALID_ARGUMENT,
                        "campaign map needs its installed package resolver",
                    )
                })?
                .show_new_game_map(
                    &mut record.runtime,
                    RuntimeGuard {
                        runtime_id: handle,
                        generation,
                    },
                )
                .map(|_| ())
                .map_err(map_transition_error)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_loading_presentation_json_v1(
    handle: u64,
    generation: u64,
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    boundary(|| {
        with_runtime(handle, |record| {
            let value = record.loading_presentation(handle, generation)?;
            let observation = serde_json::json!({
                "identity": record.runtime.identity(),
                "value": value,
            });
            super::game_session_abi::write_json(&observation, buffer, capacity, out_length)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_active_entry_presentation_json_v1(
    handle: u64,
    generation: u64,
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    boundary(|| {
        with_runtime(handle, |record| {
            record.validate_transition(handle, generation, RuntimeMode::Playing)?;
            let resolver = record.resolver.as_ref().ok_or_else(|| {
                AbiError::new(MC_STATUS_INVALID_ARGUMENT,
                    "MISSING_RUNTIME_RESOLVER: active entry presentation requires installed packages")
            })?;
            let value = resolver
                .active_entry_presentation(
                    &record.runtime,
                    RuntimeGuard {
                        runtime_id: handle,
                        generation,
                    },
                )
                .map_err(map_transition_error)?;
            let observation =
                serde_json::json!({"identity": record.runtime.identity(), "value": value});
            super::game_session_abi::write_json(&observation, buffer, capacity, out_length)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_cancel_loading_v1(handle: u64, generation: u64) -> i32 {
    boundary(|| with_runtime_mut(handle, |record| record.cancel_loading(handle, generation)))
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
pub extern "C" fn mc_game_runtime_paused_frame_timed_v1(
    handle: u64,
    runtime_generation: u64,
    session_generation: u64,
    expected_tick: u64,
    elapsed_centiseconds: u32,
    show_objective: u8,
) -> i32 {
    boundary(|| {
        if show_objective > 1 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "show_objective must be zero or one",
            ));
        }
        with_runtime_mut(handle, |runtime| {
            runtime.paused_frame_timed(
                handle,
                runtime_generation,
                session_generation,
                expected_tick,
                elapsed_centiseconds,
                show_objective != 0,
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_accept_transition_v1(
    handle: u64,
    runtime_generation: u64,
    session_generation: u64,
    expected_tick: u64,
) -> i32 {
    boundary(|| {
        with_runtime_mut(handle, |runtime| {
            runtime.accept_transition(
                handle,
                runtime_generation,
                session_generation,
                expected_tick,
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_finish_mission_if_ready_v1(
    handle: u64,
    runtime_generation: u64,
    session_generation: u64,
    expected_tick: u64,
) -> i32 {
    boundary(|| {
        with_runtime_mut(handle, |runtime| {
            runtime.finish_mission_if_ready(
                handle,
                runtime_generation,
                session_generation,
                expected_tick,
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_preferences_command_json_v1(
    handle: u64,
    generation: u64,
    request: *const u8,
    request_len: u32,
) -> i32 {
    boundary(|| {
        if request_len > 64 * 1024 {
            return Err(AbiError::new(
                MC_STATUS_BOUNDS,
                "preferences JSON exceeds 64KiB",
            ));
        }
        let json = utf8_input(request, request_len, "preferences command")?;
        let command = serde_json::from_str(&json).map_err(|error| {
            AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                format!("invalid preferences command: {error}"),
            )
        })?;
        with_runtime_mut(handle, |record| {
            let before = record.runtime.identity().player_preferences.preferences;
            record
                .runtime
                .preferences_command(
                    RuntimeGuard {
                        runtime_id: handle,
                        generation,
                    },
                    command,
                )
                .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error))?;
            record
                .runtime
                .apply_current_dynamic_lighting_preference(RuntimeGuard {
                    runtime_id: handle,
                    generation,
                })
                .map_err(map_transition_error)?;
            let after = record.runtime.identity().player_preferences.preferences;
            if before.sound_enabled != after.sound_enabled {
                record.reset_audio()?;
            }
            if before.music_enabled != after.music_enabled {
                if let Some(music) = record
                    .campaign_presentation
                    .as_mut()
                    .and_then(|r| r.day_music.as_mut())
                {
                    if after.music_enabled {
                        music.restart();
                    } else {
                        music.stop();
                    }
                }
            }
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_media_command_json_v1(
    handle: u64,
    generation: u64,
    request: *const u8,
    request_len: u32,
) -> i32 {
    boundary(|| {
        if request_len > 256 * 1024 {
            return Err(AbiError::new(
                MC_STATUS_BOUNDS,
                "frontend media JSON exceeds 256KiB",
            ));
        }
        let json = utf8_input(request, request_len, "frontend media command")?;
        let command = serde_json::from_str(&json).map_err(|error| {
            AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                format!("invalid frontend media command: {error}"),
            )
        })?;
        with_runtime_mut(handle, |record| {
            record
                .runtime
                .frontend_media_command(
                    RuntimeGuard {
                        runtime_id: handle,
                        generation,
                    },
                    command,
                )
                .map_err(map_transition_error)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_campaign_menu_command_json_v1(
    handle: u64,
    generation: u64,
    request: *const u8,
    request_len: u32,
) -> i32 {
    boundary(|| {
        if request_len > MAX_INSTALLED_PACKAGE_SET_JSON_BYTES {
            return Err(AbiError::new(
                MC_STATUS_BOUNDS,
                "campaign action JSON exceeds the negotiated bound",
            ));
        }
        let request = utf8_input(request, request_len, "campaign action JSON")?;
        with_runtime_mut(handle, |runtime| {
            runtime.campaign_menu_command(handle, generation, &request)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_completion_transition_receipt_json_v1(
    handle: u64,
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    boundary(|| {
        with_runtime(handle, |runtime| {
            super::game_session_abi::write_json(
                runtime.completion_transition_receipt()?,
                buffer,
                capacity,
                out_length,
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_boundary_transition_receipt_json_v1(
    handle: u64,
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    boundary(|| {
        with_runtime(handle, |record| {
            record.ensure_open()?;
            record
                .completion_transition_receipt
                .as_ref()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_INVALID_ARGUMENT,
                        "NO_BOUNDARY_TRANSITION_RECEIPT: no runtime boundary has been accepted",
                    )
                })?
                .write_json(buffer, capacity, out_length)
        })
    })
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

    #[test]
    fn runtime_selected_bank_preflight_preserves_old_binding_on_wrong_destination() {
        let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let request = serde_json::json!({
            "request_contract": INSTALLED_PACKAGE_SET_CONTRACT,
            "packages": [
                {"package_dir": generated.join("mc-bhq8-level1-collision-v5-night"), "owner_cache_path": generated.join("mc-oi68-level1-fullbank-v2-20261029")},
                {"package_dir": generated.join("mc-level2-vault-successor-closure-20261025"), "owner_cache_path": generated.join("mc-oi68-level2-fullbank-v2-20261029")},
                {"package_dir": generated.join("level3-day-block32-campaign-recipe-v2"), "owner_cache_path": generated.join("mc-oi68-level3-fullbank-v2-20261029")}
            ]
        }).to_string();
        let mut record = RuntimeRecord::title(9921).unwrap();
        record
            .bootstrap_package_set(9921, 1, request.clone(), request.len() as u32)
            .unwrap();
        let loading = record.runtime.identity();
        record.open_level(9921, loading.generation).unwrap();
        let playing = record.runtime.identity();
        let before = record.session().unwrap().snapshot();
        let banks = record
            .installed_inputs
            .iter()
            .map(|input| input.sound_bank.clone())
            .collect::<Vec<_>>();
        assert!(banks.iter().all(Option::is_some));
        assert_eq!(
            banks[0].as_ref().unwrap().selection(),
            CampaignSelection::LEVEL1
        );
        assert_eq!(
            banks[1].as_ref().unwrap().selection(),
            CampaignSelection::LEVEL2
        );
        let original_cache = record.installed_inputs[0].campaign.owner_cache.clone();
        record.installed_inputs[0].campaign.owner_cache =
            record.installed_inputs[1].campaign.owner_cache.clone();
        assert!(
            preflight_sound_banks(
                &mut record.installed_inputs,
                record.resolver.as_ref().unwrap(),
            )
            .is_err()
        );
        assert_eq!(record.runtime.identity(), playing);
        assert_eq!(record.session().unwrap().snapshot(), before);
        assert_eq!(
            record
                .installed_inputs
                .iter()
                .map(|input| input.sound_bank.clone())
                .collect::<Vec<_>>(),
            banks
        );
        record.installed_inputs[0].campaign.owner_cache = original_cache;
        record.pause(9921, playing.generation).unwrap();
        assert!(record.audio.voices.iter().all(Option::is_none));
        assert!(record.audio.commands.is_empty());
        assert_eq!(record.installed_inputs[0].sound_bank, banks[0]);
    }

    #[test]
    fn installed_day_environment_and_audio_companion_keep_exact_package_bindings() {
        let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let request = serde_json::json!({
            "request_contract": INSTALLED_PACKAGE_SET_CONTRACT,
            "packages": [{
                "package_dir": generated.join("mc-level2-vault-successor-closure-20261025"),
                "owner_cache_path": generated.join("mc-level2-vault-successor-environment-20261025"),
                "presentation_cache_path": generated.join("level2-vault-successor-owner-cache-20261025/components/material")
            }]
        }).to_string();
        let inputs = installed_package_set_inputs(request.clone(), request.len() as u32).unwrap();
        let input = &inputs[0];
        let selection = CampaignSelection::LEVEL2;
        let campaign = sim_core::campaign::CampaignSession::open_with_owner_cache(
            &input.campaign.package_root,
            input.campaign.owner_cache.as_ref().unwrap(),
            selection,
        )
        .unwrap();
        let assets = load_optional_day_frame_assets(
            input.campaign.owner_cache.as_deref().and_then(Path::to_str),
            campaign.inspection(),
            Some(&campaign.definition().package_sha256),
        )
        .unwrap()
        .unwrap();
        assert!(assets.environment_resources().day_exit_tables().is_some());
        let music = super::super::campaign_audio::load_campaign_music(
            input.presentation_cache.as_deref(),
            selection,
            campaign.inspection().package_sha256(),
            &campaign.definition().package_sha256,
        )
        .unwrap()
        .unwrap();
        assert_eq!(music.track, 2);
        assert!(music.loaded && music.playing);
        assert!(
            super::super::campaign_audio::load_campaign_music(
                input.presentation_cache.as_deref(),
                selection,
                campaign.inspection().package_sha256(),
                "wrong-package",
            )
            .is_err()
        );
    }

    fn level_one_package() -> PathBuf {
        std::env::var_os("MC_CAMPAIGN_LEVEL1_PACKAGE").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-bhq8-level1-collision-v5-night")
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
            "carpet-flight-game-runtime/v12"
        );
        assert_eq!(
            contract["transport_contract"],
            "carpet-flight-native-game-runtime-json/v12"
        );
        assert_eq!(contract["transport_version"], 12);
        assert_eq!(
            contract["definition_contract"],
            "carpet-flight-game-definition/v11"
        );
        assert_eq!(contract["semantic_contract_version"], "36.0");
        assert_eq!(
            contract["modes"],
            serde_json::json!(["title", "campaign", "loading", "playing", "paused"])
        );
        assert_eq!(
            contract["transitions"],
            serde_json::json!([
                "bootstrap_package_set",
                "restart_new_game",
                "show_new_game_map",
                "open_level",
                "cancel_loading",
                "load_replay_save",
                "pause",
                "paused_frame",
                "resume",
                "accept_completion_transition",
                "finish_mission_if_ready",
                "campaign_menu_command",
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
                "save_contract": "carpet-flight-mc2-campaign-replay-save/v6",
                "save_transport": "mc_game_runtime_save_replay_json_v1",
                "load_transport": "mc_game_runtime_load_replay_json_v1",
                "warm_load_transport": "mc_game_runtime_load_replay_from_runtime_json_v1",
                "load_package_authority": "complete_unordered_resource_set",
                "client_semantics": "opaque_utf8_json"
            })
        );
        assert_eq!(
            contract["completion_transition"],
            serde_json::json!({
                "receipt_contract": "carpet-flight-completion-transition-receipt/v2",
                "accept_transport": "mc_game_runtime_accept_transition_v1",
                "receipt_transport": "mc_game_runtime_completion_transition_receipt_json_v1",
                "destination_authority": "rust_owned_semantic_flow",
                "package_authority": "installed_package_resolver",
                "presentation_sequence": "ordered_rust_owned_semantic_cues",
                "guards": ["runtime_generation", "session_generation", "expected_tick"]
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
                "playing_and_paused": "active_runtime_flow",
                "presentation_cue": "identity.presentation"
            })
        );

        let mut handle = 0_u64;
        assert_eq!(mc_game_runtime_create_v1(&raw mut handle), MC_STATUS_OK);
        assert_ne!(handle, 0);
        let title = identity(handle);
        assert_eq!(title["mode"], "title");
        assert_eq!(title["generation"], "1");
        assert_eq!(
            title["presentation"],
            serde_json::json!({
                "cue_id": "presentation.campaign.title",
                "phase": "title"
            })
        );
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

        // The ordinary first step needs the bank-bearing owner package as well
        // as its owner cache. A cache cannot supply a missing package bank.
        let level_one = level_one_package();
        let owner_cache = std::env::var_os("MC_CAMPAIGN_LEVEL1_CACHE").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc2-level1-night-cache-v2")
            },
            PathBuf::from,
        );
        let packages = serde_json::to_vec(&serde_json::json!({
            "request_contract": INSTALLED_PACKAGE_SET_CONTRACT,
            "packages": [
                { "package_dir": level_three_package(), "owner_cache_path": null },
                { "package_dir": level_one, "owner_cache_path": owner_cache },
                { "package_dir": level_two_package(), "owner_cache_path": null }
            ]
        }))
        .unwrap();
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
            loading["presentation"],
            serde_json::json!({
                "cue_id": "presentation.magic_carpet_2.night_spire.loading",
                "phase": "loading",
                "level_id": "level.magic_carpet_2.night_spire"
            })
        );
        assert_eq!(
            loading["pending_definition"]["runtime_flow"]["game_id"],
            "game.magic_carpet_2"
        );
        assert_eq!(
            loading["pending_definition"]["runtime_flow"]["bound_level_id"],
            "level.magic_carpet_2.night_spire"
        );
        assert!(loading.get("active_runtime_flow").is_none());
        assert_eq!(
            mc_game_runtime_open_level_v1(handle, 2),
            MC_STATUS_OK,
            "{}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        let playing = identity(handle);
        assert_eq!(playing["mode"], "playing");
        assert_eq!(playing["generation"], "3");
        assert_eq!(playing["session"]["session_id"], handle.to_string());
        assert_eq!(playing["session"]["generation"], "1");
        assert_eq!(playing["session"]["tick"], "0");
        assert_eq!(
            playing["presentation"],
            serde_json::json!({
                "cue_id": "presentation.magic_carpet_2.night_spire.entry",
                "phase": "level_entry",
                "level_id": "level.magic_carpet_2.night_spire"
            })
        );
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
        let mut world = McCampaignWorldInfoV2::default();
        assert_eq!(
            mc_native_campaign_world_info(
                handle,
                &raw mut world,
                struct_size::<McCampaignWorldInfoV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!((world.external_level, world.internal_level), (1, 0));
        let mut entity_count = 0_u32;
        assert_eq!(
            mc_native_campaign_entity_count(handle, &raw mut entity_count),
            MC_STATUS_OK
        );
        assert_eq!(entity_count, world.entity_count);
        assert!(entity_count > 0);
        let check_shared_inspection = || {
            with_runtime_mut(handle, |record| {
                let before = record.campaign_snapshot()?;
                let authoritative = record
                    .session()?
                    .backend()
                    .campaign_presentation_source()
                    .inspection_arc();
                // The first read refreshes the dirty adapter; the second
                // reuses it. Both must retain the authoritative allocation.
                for _ in 0..2 {
                    let presentation = record.take_campaign_presentation_record()?;
                    let super::super::campaign_abi::CampaignSessionKind::Campaign {
                        inspection, ..
                    } = &presentation.kind
                    else {
                        panic!("runtime presentation retains its campaign variant");
                    };
                    assert!(Arc::ptr_eq(inspection, &authoritative));
                    record.retain_campaign_presentation_record(presentation);
                }
                assert_eq!(record.campaign_snapshot()?, before);
                Ok(())
            })
            .unwrap();
        };
        check_shared_inspection();
        let command = br#"{"longitudinal":"accelerate"}"#;
        assert_eq!(
            mc_game_session_step_json_v1(handle, 1, 0, command.as_ptr(), command.len() as u32,),
            MC_STATUS_OK
        );
        let stepped = read_json(|buffer, capacity, length| {
            mc_game_session_snapshot_json_v1(handle, buffer, capacity, length)
        });
        assert_eq!(stepped["identity"]["tick"], "1");
        check_shared_inspection();

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
            installed_inputs: Vec::new(),
            campaign_presentation: None,
            campaign_presentation_dirty: true,
            audio: super::super::campaign_audio_state::CampaignRuntimeAudio::default(),
            completion_transition_receipt: None,
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
    fn loading_presentation_and_cancel_use_the_exact_guarded_destination() {
        let mut handle = 0;
        assert_eq!(mc_game_runtime_create_v1(&raw mut handle), MC_STATUS_OK);
        assert_eq!(
            bootstrap_package_set(
                handle,
                1,
                &[
                    level_three_package(),
                    level_one_package(),
                    level_two_package()
                ]
            ),
            MC_STATUS_OK
        );
        let loading = identity(handle);
        let first = read_json(|buffer, capacity, length| {
            mc_game_runtime_loading_presentation_json_v1(handle, 2, buffer, capacity, length)
        });
        assert_eq!(first["identity"], loading);
        assert_eq!(first["value"]["schema"], "mc2-loading-presentation/v1");
        assert_eq!(
            first["value"]["level_id"],
            loading["presentation"]["level_id"]
        );
        assert_eq!(first["value"]["cue_id"], loading["presentation"]["cue_id"]);
        for field in [
            "package_graph_hash",
            "definition_hash",
            "definition_revision",
        ] {
            assert_eq!(first["value"][field], loading["pending_definition"][field]);
        }
        assert!(first["value"]["definition_revision"].is_string());
        assert_eq!(first["value"]["environment"], "night");
        assert_eq!(first["value"]["artwork_profile"], "ordinary");
        let manifest: Value =
            serde_json::from_slice(&fs::read(level_one_package().join("manifest.json")).unwrap())
                .unwrap();
        assert_eq!(
            first["value"]["source_package_sha256"],
            manifest["package_sha256"]
        );
        assert_eq!(
            first["value"]["runtime_capture_sha256"],
            manifest["files"]["runtime.bin"]["sha256"]
        );
        assert_eq!(
            first["value"]["owner_export_package_sha256"],
            manifest["owner_export_package_sha256"]
        );
        assert_eq!(
            read_json(|buffer, capacity, length| {
                mc_game_runtime_loading_presentation_json_v1(handle, 2, buffer, capacity, length)
            }),
            first
        );
        assert_eq!(identity(handle), loading);
        assert!(loading.get("session").is_none());

        let mut length = 123;
        assert_eq!(
            mc_game_runtime_loading_presentation_json_v1(
                handle,
                1,
                std::ptr::null_mut(),
                0,
                &raw mut length
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(length, 123);
        assert_eq!(
            mc_game_runtime_cancel_loading_v1(handle, 1),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(identity(handle), loading);
        assert_eq!(mc_game_runtime_cancel_loading_v1(handle, 2), MC_STATUS_OK);
        let title = identity(handle);
        assert_eq!(title["mode"], "title");
        assert_eq!(title["generation"], "3");
        assert!(title.get("session").is_none());
        assert_eq!(
            mc_game_runtime_cancel_loading_v1(handle, 2),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(
            mc_game_runtime_cancel_loading_v1(handle, 3),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(identity(handle), title);

        assert_eq!(mc_game_runtime_restart_new_game_v1(handle, 3), MC_STATUS_OK);
        let retry = read_json(|buffer, capacity, length| {
            mc_game_runtime_loading_presentation_json_v1(handle, 4, buffer, capacity, length)
        });
        assert_eq!(retry["value"], first["value"]);
        assert_eq!(mc_game_runtime_open_level_v1(handle, 4), MC_STATUS_OK);
        assert_eq!(identity(handle)["mode"], "playing");
        assert_eq!(mc_game_runtime_destroy_v1(handle), MC_STATUS_OK);
    }

    #[test]
    fn normal_new_game_map_exposes_source_briefing_without_earned_completion() {
        let mut handle = 0;
        assert_eq!(mc_game_runtime_create_v1(&raw mut handle), MC_STATUS_OK);
        assert_eq!(
            bootstrap_package_set(
                handle,
                1,
                &[
                    level_one_package(),
                    level_two_package(),
                    level_three_package()
                ]
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_game_runtime_show_new_game_map_v1(handle, 2),
            MC_STATUS_OK
        );
        let map = identity(handle);
        assert_eq!(map["mode"], "campaign");
        assert_eq!(map["campaign_menu"]["phase"], "mission_select");
        assert!(map["campaign_menu"].get("completed_level_id").is_none());
        assert_eq!(
            map["campaign_menu"]["completed_levels"],
            serde_json::json!([])
        );
        assert_eq!(map["campaign_menu"]["briefing_speech"]["track"], 1);
        assert_eq!(map["campaign_menu"]["briefing_speech"]["segment"], 0);
        let saved = with_runtime(handle, |record| record.save_replay(handle, 3, 0, 0)).unwrap();
        assert!(with_runtime(handle, |record| record.save_replay(handle, 3, 1, 0)).is_err());
        assert!(with_runtime(handle, |record| record.save_replay(handle, 3, 0, 1)).is_err());
        assert!(with_runtime(handle, |record| record.save_replay(handle, 2, 0, 0)).is_err());
        assert_eq!(
            identity(handle),
            map,
            "map save and rejected guards are observational"
        );
        let mut restored = 0;
        assert_eq!(mc_game_runtime_create_v1(&raw mut restored), MC_STATUS_OK);
        let packages = installed_package_set_json(&[
            level_one_package(),
            level_two_package(),
            level_three_package(),
        ]);
        let mut warm = 0;
        assert_eq!(mc_game_runtime_create_v1(&raw mut warm), MC_STATUS_OK);
        let warm_title = identity(warm);
        assert_eq!(
            mc_game_runtime_load_replay_from_runtime_json_v1(
                warm,
                1,
                handle,
                2,
                packages.as_ptr(),
                packages.len() as u32,
                saved.as_ptr(),
                saved.len() as u32,
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(identity(warm), warm_title);
        assert_eq!(
            identity(handle),
            map,
            "rejected warm load cannot mutate its source owner"
        );
        assert_eq!(
            mc_game_runtime_load_replay_from_runtime_json_v1(
                warm,
                1,
                handle,
                3,
                packages.as_ptr(),
                packages.len() as u32,
                saved.as_ptr(),
                saved.len() as u32,
            ),
            MC_STATUS_OK
        );
        assert_eq!(identity(warm)["campaign_menu"], map["campaign_menu"]);
        assert_eq!(
            identity(handle),
            map,
            "successful replacement leaves old owner alive until host attachment"
        );
        assert_eq!(mc_game_runtime_destroy_v1(warm), MC_STATUS_OK);
        assert_eq!(
            mc_game_runtime_load_replay_json_v1(
                restored,
                1,
                packages.as_ptr(),
                packages.len() as u32,
                saved.as_ptr(),
                saved.len() as u32,
            ),
            MC_STATUS_OK
        );
        assert_eq!(identity(restored)["mode"], "campaign");
        assert_eq!(identity(restored)["campaign_menu"], map["campaign_menu"]);
        assert_eq!(mc_game_runtime_destroy_v1(restored), MC_STATUS_OK);
        assert_eq!(
            mc_game_runtime_open_level_v1(handle, 3),
            MC_STATUS_INVALID_ARGUMENT
        );
        let command = br#"{"action":"start_mission"}"#;
        assert_eq!(
            mc_game_runtime_campaign_menu_command_json_v1(
                handle,
                3,
                command.as_ptr(),
                command.len() as u32
            ),
            MC_STATUS_OK
        );
        assert_eq!(identity(handle)["mode"], "loading");
        assert_eq!(mc_game_runtime_open_level_v1(handle, 4), MC_STATUS_OK);
        assert_eq!(mc_game_runtime_destroy_v1(handle), MC_STATUS_OK);
    }

    #[test]
    fn speech_preference_is_guarded_and_strictly_replayed_before_paused_phase() {
        let packages = [
            level_one_package(),
            level_two_package(),
            level_three_package(),
        ];
        let package_set = installed_package_set_json(&packages);
        let mut handle = 0;
        assert_eq!(mc_game_runtime_create_v1(&raw mut handle), MC_STATUS_OK);
        assert_eq!(bootstrap_package_set(handle, 1, &packages), MC_STATUS_OK);
        assert_eq!(mc_game_runtime_open_level_v1(handle, 2), MC_STATUS_OK);
        let opened_save =
            with_runtime(handle, |record| record.save_replay(handle, 3, 1, 0)).unwrap();
        let opened_history: Value = serde_json::from_slice(&opened_save).unwrap();
        assert_eq!(opened_history["operations"], serde_json::json!([]));
        assert_eq!(mc_game_runtime_pause_v1(handle, 3), MC_STATUS_OK);
        let paused_save =
            with_runtime(handle, |record| record.save_replay(handle, 4, 2, 0)).unwrap();
        let paused_history: Value = serde_json::from_slice(&paused_save).unwrap();
        assert_eq!(
            paused_history["operations"],
            serde_json::json!([{"operation": "pause"}])
        );
        // Pause is already recorded. Preview and rejection must preserve the
        // entire replay, before one speech command and two paused phases append.
        let initial_replay_operations = with_runtime(handle, |record| {
            Ok(record.session()?.backend().replay_operation_count())
        })
        .unwrap();
        assert_eq!(
            initial_replay_operations,
            paused_history["operations"].as_array().unwrap().len()
        );
        let preview = br#"{"operation":"preview_audio","sound_enabled":true,"music_enabled":true,"speech_enabled":false,"sound_volume":127,"music_volume":127}"#;
        assert_eq!(
            mc_game_runtime_preferences_command_json_v1(
                handle,
                4,
                preview.as_ptr(),
                preview.len() as u32
            ),
            MC_STATUS_OK
        );
        let preview_save =
            with_runtime(handle, |record| record.save_replay(handle, 4, 2, 0)).unwrap();
        assert_eq!(preview_save, paused_save);
        assert_ne!(
            mc_game_runtime_paused_frame_v1(handle, 4, 2, 1),
            MC_STATUS_OK
        );
        let rejected_save = with_runtime(handle, |record| {
            assert!(record.session()?.backend().speech_enabled());
            assert_eq!(
                record.session()?.backend().replay_operation_count(),
                initial_replay_operations
            );
            record.save_replay(handle, 4, 2, 0)
        })
        .unwrap();
        assert_eq!(rejected_save, paused_save);
        for _ in 0..2 {
            assert_eq!(
                mc_game_runtime_paused_frame_v1(handle, 4, 2, 0),
                MC_STATUS_OK
            );
        }
        let save = with_runtime(handle, |record| {
            let state = record
                .session()?
                .backend()
                .campaign_presentation_source()
                .simulation()
                .runtime();
            assert_eq!(state.objective_presentation_phase, 0);
            assert_eq!(state.objective_textbox_ticks, 200);
            record.save_replay(handle, 4, 2, 0)
        })
        .unwrap();
        let accepted_history: Value = serde_json::from_slice(&save).unwrap();
        assert_eq!(
            accepted_history["operations"],
            serde_json::json!([
                {"operation": "pause"},
                {"operation": "speech_preference", "enabled": false},
                {"operation": "paused_frame"},
                {"operation": "paused_frame"}
            ])
        );
        let mut restored = 0;
        assert_eq!(mc_game_runtime_create_v1(&raw mut restored), MC_STATUS_OK);
        with_runtime_mut(restored, |record| {
            record.load_replay(
                restored,
                1,
                String::from_utf8(package_set.clone()).unwrap(),
                package_set.len() as u32,
                &save,
            )
        })
        .unwrap();
        let restored_save = with_runtime(restored, |record| {
            assert!(!record.session()?.backend().speech_enabled());
            assert_eq!(
                record.session()?.backend().replay_operation_count(),
                initial_replay_operations + 3
            );
            record.save_replay(restored, 2, 1, 0)
        })
        .unwrap();
        assert_eq!(restored_save, save);
        assert_eq!(mc_game_runtime_destroy_v1(handle), MC_STATUS_OK);
        assert_eq!(mc_game_runtime_destroy_v1(restored), MC_STATUS_OK);
    }

    #[test]
    fn runtime_pause_ends_source_audio_pool_and_resume_restarts_loaded_music() {
        let mut handle = 0;
        assert_eq!(mc_game_runtime_create_v1(&raw mut handle), MC_STATUS_OK);
        assert_eq!(
            bootstrap_package_set(
                handle,
                1,
                &[
                    level_one_package(),
                    level_two_package(),
                    level_three_package()
                ]
            ),
            MC_STATUS_OK
        );
        assert_eq!(mc_game_runtime_open_level_v1(handle, 2), MC_STATUS_OK);
        for tick in 0..10 {
            assert_eq!(
                mc_game_session_step_json_v1(handle, 1, tick, b"{}".as_ptr(), 2),
                MC_STATUS_OK
            );
        }
        let command = with_runtime(handle, |record| {
            Ok(*record
                .audio
                .commands
                .values()
                .find(|command| command.sound_id == 41)
                .expect("real phase7 sound request"))
        })
        .unwrap();
        assert_eq!(command.volume_q15, 0x7fff);
        assert_eq!(command.pan_q16, 0x7fff);
        assert_eq!(command.play_type, 1);
        assert_eq!(command.action, MC_AUDIO_ACTION_START);
        assert_ne!(command.voice_index, MC_AUDIO_VOICE_NONE);
        let check_effect_page = |generation| {
            let query = McEventQueryV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McEventQueryV2>(),
                generation,
                cursor: command.event_sequence - 1,
                reserved: [0; 4],
            };
            let mut rows = [McEventV2::default(); 1];
            let mut batch = McEventBatchV2::default();
            let status = super::super::campaign_abi::mc_native_campaign_events_copy_since(
                handle,
                &raw const query,
                rows.as_mut_ptr(),
                1,
                struct_size::<McEventV2>(),
                &raw mut batch,
                struct_size::<McEventBatchV2>(),
            );
            if status == MC_STATUS_OK {
                assert_eq!(batch.generation, generation);
                assert_eq!(batch.written, 1);
                assert_eq!(rows[0].generation, generation);
                assert_eq!(rows[0].sequence, command.event_sequence);
                assert_eq!(rows[0].state_hash, command.state_hash);
            }
            status
        };
        assert_eq!(check_effect_page(1), MC_STATUS_OK);
        with_runtime_mut(handle, |record| {
            // Use the actual loaded map's lifecycle, independent of a sound card.
            let presentation = record.take_campaign_presentation_record()?;
            record.retain_campaign_presentation_record(presentation);
            Ok(())
        })
        .unwrap();
        assert_eq!(mc_game_runtime_pause_v1(handle, 3), MC_STATUS_OK);
        assert_eq!(check_effect_page(1), MC_STATUS_GENERATION_MISMATCH);
        assert_eq!(check_effect_page(2), MC_STATUS_OK);
        let mut paused = McAudioCommandV2::default();
        assert_eq!(
            mc_game_runtime_audio_command_for_event_v1(
                handle,
                2,
                command.event_sequence,
                &raw mut paused,
                struct_size::<McAudioCommandV2>()
            ),
            MC_STATUS_BOUNDS
        );
        with_runtime(handle, |record| {
            assert!(record.audio.voices.iter().all(Option::is_none));
            assert!(record.audio.commands.is_empty());
            if let Some(music) = record
                .campaign_presentation
                .as_ref()
                .and_then(|r| r.day_music)
            {
                assert!(!music.playing);
                assert!(!music.envelope.timer_active);
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(
            mc_game_runtime_audio_voice_complete_v1(
                handle,
                2,
                command.voice_index,
                command.event_sequence + 1
            ),
            MC_STATUS_BOUNDS
        );
        assert_eq!(
            mc_game_runtime_audio_voice_complete_v1(
                handle,
                2,
                command.voice_index,
                command.event_sequence
            ),
            MC_STATUS_BOUNDS
        );
        assert_eq!(mc_game_runtime_resume_v1(handle, 4), MC_STATUS_OK);
        with_runtime(handle, |record| {
            assert!(record.audio.voices.iter().all(Option::is_none));
            if let Some(music) = record
                .campaign_presentation
                .as_ref()
                .and_then(|r| r.day_music)
            {
                assert!(music.playing);
                assert_eq!(music.start_count, 2);
                assert_eq!(music.volume, 127);
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(mc_game_runtime_destroy_v1(handle), MC_STATUS_OK);
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
        assert_eq!(
            mc_game_runtime_open_level_v1(source, 2),
            MC_STATUS_OK,
            "{}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
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
            "carpet-flight-mc2-campaign-replay-save/v6"
        );
        assert_eq!(save_json["completed_levels"], serde_json::json!([]));
        let operations = save_json["operations"].as_array().unwrap();
        assert_eq!(operations.len(), 6);
        assert_eq!(operations[3]["operation"], "pause");
        assert_eq!(operations[4]["operation"], "paused_frame");
        assert_eq!(operations[5]["operation"], "paused_frame");
        assert_eq!(save_json["paused"], true);

        let mut restored = 0_u64;
        assert_eq!(mc_game_runtime_create_v1(&raw mut restored), MC_STATUS_OK);
        let restored_title = identity(restored);
        // The native save receipt covers earned campaign progress. Neither
        // a fabricated unlock nor an omitted v6 frontier may publish a
        // candidate session, consume the resolver, or alter the title guard.
        let mut fabricated_progress = save_json.clone();
        fabricated_progress["completed_levels"] =
            serde_json::json!([source_identity["active_runtime_flow"]["bound_level_id"]]);
        let mut missing_progress = save_json.clone();
        missing_progress
            .as_object_mut()
            .unwrap()
            .remove("completed_levels");
        for invalid in [fabricated_progress, missing_progress] {
            let bytes = serde_json::to_vec(&invalid).unwrap();
            assert_eq!(
                mc_game_runtime_load_replay_json_v1(
                    restored,
                    1,
                    package_set.as_ptr(),
                    package_set.len() as u32,
                    bytes.as_ptr(),
                    bytes.len() as u32,
                ),
                MC_STATUS_VALIDATION_FAILED
            );
            assert_eq!(identity(restored), restored_title);
        }
        let mut stale_save_json = save_json.clone();
        stale_save_json["save_contract"] =
            Value::String("carpet-flight-mc2-campaign-replay-save/v4".to_owned());
        let stale_save = serde_json::to_vec(&stale_save_json).unwrap();
        assert_eq!(
            mc_game_runtime_load_replay_json_v1(
                restored,
                1,
                package_set.as_ptr(),
                package_set.len() as u32,
                stale_save.as_ptr(),
                stale_save.len() as u32,
            ),
            MC_STATUS_VALIDATION_FAILED
        );
        assert_eq!(identity(restored), restored_title);
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
        let restored_save = read_bytes(|buffer, capacity, length| {
            mc_game_runtime_save_replay_json_v1(restored, 2, 1, 3, buffer, capacity, length)
        });
        let restored_save_json: Value = serde_json::from_slice(&restored_save).unwrap();
        assert_eq!(
            restored_save_json["completed_levels"],
            save_json["completed_levels"]
        );
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
    fn runtime_completion_transition_guards_reject_atomically_before_completion() {
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
        let before = identity(handle);
        let runtime_generation = before["generation"].as_str().unwrap().parse().unwrap();
        let session_generation = before["session"]["generation"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        let tick = before["session"]["tick"].as_str().unwrap().parse().unwrap();

        assert_eq!(
            mc_game_runtime_finish_mission_if_ready_v1(
                handle,
                runtime_generation,
                session_generation,
                tick
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            identity(handle),
            before,
            "an unfinished boundary neither advances gameplay nor changes mode"
        );
        assert_ne!(
            mc_game_runtime_finish_mission_if_ready_v1(
                handle,
                runtime_generation,
                session_generation,
                tick + 1
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            identity(handle),
            before,
            "a stale finish boundary is atomic"
        );

        assert_eq!(
            mc_game_runtime_completion_transition_receipt_json_v1(
                handle,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
            ),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            mc_game_runtime_accept_transition_v1(
                handle,
                runtime_generation,
                session_generation,
                tick,
            ),
            MC_STATUS_INVALID_ARGUMENT,
            "an in-progress authoritative session cannot be completed by the client"
        );
        assert_eq!(identity(handle), before);

        assert_eq!(
            mc_game_runtime_accept_transition_v1(
                handle,
                runtime_generation - 1,
                session_generation,
                tick,
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(identity(handle), before);
        assert_eq!(
            mc_game_runtime_accept_transition_v1(
                handle,
                runtime_generation,
                session_generation + 1,
                tick,
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(identity(handle), before);
        assert_eq!(
            mc_game_runtime_accept_transition_v1(
                handle,
                runtime_generation,
                session_generation,
                tick + 1,
            ),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(identity(handle), before);
        assert_eq!(
            mc_game_runtime_completion_transition_receipt_json_v1(
                handle,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
            ),
            MC_STATUS_INVALID_ARGUMENT,
            "rejected transitions must not manufacture a receipt"
        );
        assert_eq!(mc_game_runtime_destroy_v1(handle), MC_STATUS_OK);
    }

    #[test]
    fn runtime_completion_receipt_json_preserves_ordered_semantic_cues() {
        use sim_core::{
            game_definition::{
                RuntimeFlowDestination, RuntimeFlowNodeId, SemanticLevelId,
                SemanticPresentationCueId,
            },
            game_runtime::{RuntimePresentationCue, RuntimePresentationPhase},
        };

        let mut handle = 0_u64;
        assert_eq!(mc_game_runtime_create_v1(&raw mut handle), MC_STATUS_OK);
        let title = with_runtime(handle, |runtime| Ok(runtime.runtime.identity())).unwrap();
        let completed_level = SemanticLevelId::parse("level.magic_carpet_2.revealed_way").unwrap();
        let destination = RuntimeFlowDestination::Title {
            node_id: RuntimeFlowNodeId::parse("flow.magic_carpet_2.return_to_title").unwrap(),
        };
        let completion_cue = RuntimePresentationCue {
            cue_id: SemanticPresentationCueId::parse(
                "presentation.magic_carpet_2.revealed_way.completion",
            )
            .unwrap(),
            phase: RuntimePresentationPhase::LevelCompletion,
            level_id: Some(completed_level.clone()),
            completion_destination: Some(destination.clone()),
        };
        let receipt = CompletionTransitionReceipt {
            receipt_contract: COMPLETION_TRANSITION_RECEIPT,
            before: title.clone(),
            after: title.clone(),
            source_session: sim_session::SessionIdentity {
                engine_contract: sim_session::GAME_SESSION_CONTRACT,
                package_graph_hash: "sha256:package".to_owned(),
                definition_hash: "sha256:definition".to_owned(),
                definition_revision: 1,
                session_id: handle,
                generation: 1,
                tick: 9,
            },
            completed_level_id: completed_level,
            completion_destination: destination,
            completion: sim_session::GameCompletion {
                state_hash: 0x1234,
                statistics: None,
            },
            presentation_sequence: vec![completion_cue, title.presentation.clone()],
            receipt_sha256: "0".repeat(64),
        };
        with_runtime_mut(handle, |runtime| {
            runtime.completion_transition_receipt =
                Some(BoundaryTransitionReceipt::OrdinaryCompletion {
                    ordinary_completion: receipt,
                });
            Ok(())
        })
        .unwrap();

        let encoded = read_json(|buffer, capacity, length| {
            mc_game_runtime_completion_transition_receipt_json_v1(handle, buffer, capacity, length)
        });
        assert_eq!(
            encoded["receipt_contract"],
            "carpet-flight-completion-transition-receipt/v2"
        );
        assert_eq!(
            encoded["presentation_sequence"],
            serde_json::json!([
                {
                    "cue_id": "presentation.magic_carpet_2.revealed_way.completion",
                    "phase": "level_completion",
                    "level_id": "level.magic_carpet_2.revealed_way",
                    "completion_destination": {
                        "kind": "title",
                        "node_id": "flow.magic_carpet_2.return_to_title"
                    }
                },
                {
                    "cue_id": "presentation.campaign.title",
                    "phase": "title"
                }
            ])
        );
        assert_eq!(mc_game_runtime_destroy_v1(handle), MC_STATUS_OK);
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
        let binding = read_json(|buffer, capacity, length| {
            mc_game_runtime_loading_presentation_json_v1(
                failed_open_handle,
                2,
                buffer,
                capacity,
                length,
            )
        });
        fs::remove_file(failed_open_root.join("runtime.bin")).unwrap();
        assert_eq!(
            mc_game_runtime_open_level_v1(failed_open_handle, 2),
            MC_STATUS_VALIDATION_FAILED
        );
        assert_eq!(identity(failed_open_handle), failed_open_loading);
        assert_eq!(
            read_json(|buffer, capacity, length| {
                mc_game_runtime_loading_presentation_json_v1(
                    failed_open_handle,
                    2,
                    buffer,
                    capacity,
                    length,
                )
            }),
            binding,
            "failed open retains the exact installed loading metadata for retry"
        );
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
        assert_eq!(
            mc_game_runtime_cancel_loading_v1(failed_open_handle, 2),
            MC_STATUS_OK
        );
        assert_eq!(identity(failed_open_handle)["mode"], "title");
        assert_eq!(mc_game_runtime_destroy_v1(failed_open_handle), MC_STATUS_OK);
        fs::remove_dir_all(failed_open_root).unwrap();
    }

    #[test]
    fn opened_level_three_runtime_projects_persistent_day_frame_on_same_handle() {
        let replay_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/opened-l3-runtime-replay-f1fd3b5-20261005.json");
        let replay = fs::read(replay_path)
            .expect("owner-local Level 3 replay is generated by the exact sim-core replay test");
        let level_three_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level3-day-material-cache");
        let package_set = serde_json::to_vec(&serde_json::json!({
            "request_contract": INSTALLED_PACKAGE_SET_CONTRACT,
            "packages": [
                { "package_dir": level_three_package(), "owner_cache_path": level_three_cache },
                { "package_dir": level_one_package(), "owner_cache_path": null },
                { "package_dir": level_two_package(), "owner_cache_path": null }
            ]
        }))
        .unwrap();

        let mut handle = 0_u64;
        assert_eq!(mc_game_runtime_create_v1(&raw mut handle), MC_STATUS_OK);
        assert_eq!(
            mc_game_runtime_load_replay_json_v1(
                handle,
                1,
                package_set.as_ptr(),
                package_set.len() as u32,
                replay.as_ptr(),
                replay.len() as u32,
            ),
            MC_STATUS_OK,
            "{}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        let opened = identity(handle);
        assert_eq!(opened["runtime_id"], handle.to_string());
        assert_eq!(opened["session"]["session_id"], handle.to_string());
        assert_eq!(
            opened["active_runtime_flow"]["bound_level_id"],
            "level.magic_carpet_2.revealed_way"
        );
        let generation = opened["session"]["generation"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        let tick = opened["session"]["tick"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();

        let query = McPresentationFrameQueryV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McPresentationFrameQueryV2>(),
            generation,
            tick,
            presentation_revision: tick,
            viewport_width: 320,
            viewport_height: 200,
            screen_stride: 320,
            vga_resolution: 1,
            ..McPresentationFrameQueryV2::default()
        };
        let mut generic_indices = vec![0_u8; LEVEL1_VIEW_PIXELS];
        let mut generic = McPresentationFrameInfoV2::default();
        assert_eq!(
            mc_native_campaign_presentation_frame_copy(
                handle,
                &raw const query,
                generic_indices.as_mut_ptr(),
                generic_indices.len() as u32,
                std::ptr::null_mut(),
                0,
                &raw mut generic,
                struct_size::<McPresentationFrameInfoV2>(),
            ),
            MC_STATUS_OK,
            "{}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_eq!((generic.generation, generic.tick), (generation, tick));

        let mut day = McCampaignDayIndexedFrameInfoV2::default();
        assert_eq!(
            mc_native_campaign_day_indexed_frame_render(
                handle,
                generation,
                tick,
                &raw mut day,
                struct_size::<McCampaignDayIndexedFrameInfoV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(day.index_hash, generic.index_hash);
        let mut day_indices = vec![0_u8; LEVEL1_VIEW_PIXELS];
        let mut palette = vec![0_u8; 256 * 3];
        let mut index_written = 0_u32;
        let mut palette_written = 0_u32;
        assert_eq!(
            mc_native_campaign_day_indexed_frame_copy(
                handle,
                generation,
                tick,
                day_indices.as_mut_ptr(),
                day_indices.len() as u32,
                palette.as_mut_ptr(),
                palette.len() as u32,
                &raw mut index_written,
                &raw mut palette_written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(generic_indices, day_indices);
        assert_eq!((index_written, palette_written), (64_000, 768));
        // An unchanged source may be borrowed by all frame/HUD consumers.
        // A full refresh must yield precisely the same frame and palette.
        with_runtime_mut(handle, |_| Ok(())).unwrap();
        assert_eq!(
            mc_native_campaign_presentation_frame_copy(
                handle,
                &raw const query,
                day_indices.as_mut_ptr(),
                day_indices.len() as u32,
                std::ptr::null_mut(),
                0,
                &raw mut generic,
                struct_size::<McPresentationFrameInfoV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(generic_indices, day_indices);
        assert_eq!(generic.index_hash, day.index_hash);
        assert_eq!(
            mc_native_campaign_day_indexed_frame_render(
                handle,
                generation.wrapping_add(1),
                tick,
                &raw mut day,
                struct_size::<McCampaignDayIndexedFrameInfoV2>(),
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(mc_game_runtime_destroy_v1(handle), MC_STATUS_OK);
    }

    #[test]
    fn retained_campaign_and_game_runtime_handles_are_disjoint_and_dispatch_to_their_owners() {
        // Open the shared runtime first and the retained campaign second. Before
        // typed allocation, both independent registries issued handle 1 here,
        // and campaign presentation lookup silently preferred the retained
        // Level 2 authority over this Level 3 runtime.
        let replay = fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/opened-l3-runtime-replay-f1fd3b5-20261005.json"),
        )
        .expect("owner-local Level 3 replay is generated by the exact sim-core replay test");
        let level_three_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level3-day-material-cache");
        let package_set = serde_json::to_vec(&serde_json::json!({
            "request_contract": INSTALLED_PACKAGE_SET_CONTRACT,
            "packages": [
                { "package_dir": level_three_package(), "owner_cache_path": level_three_cache },
                { "package_dir": level_one_package(), "owner_cache_path": null },
                { "package_dir": level_two_package(), "owner_cache_path": null }
            ]
        }))
        .unwrap();

        let mut runtime_handle = 0_u64;
        assert_eq!(
            mc_game_runtime_create_v1(&raw mut runtime_handle),
            MC_STATUS_OK
        );
        assert_eq!(runtime_handle & 1, 1);
        assert_eq!(
            mc_game_runtime_load_replay_json_v1(
                runtime_handle,
                1,
                package_set.as_ptr(),
                package_set.len() as u32,
                replay.as_ptr(),
                replay.len() as u32,
            ),
            MC_STATUS_OK,
            "{}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        let opened_runtime = identity(runtime_handle);
        assert_eq!(opened_runtime["mode"], "playing");
        assert_eq!(
            opened_runtime["active_runtime_flow"]["bound_level_id"],
            "level.magic_carpet_2.revealed_way"
        );

        let level_two_package = level_two_package();
        let level_two_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache");
        let package = level_two_package.to_string_lossy();
        let cache = level_two_cache.to_string_lossy();
        let selection = McCampaignSelectionV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCampaignSelectionV2>(),
            external_level: 2,
            internal_level: 1,
            map_type: 0,
            ..McCampaignSelectionV2::default()
        };
        let mut campaign_handle = 0_u64;
        assert_eq!(
            mc_native_campaign_session_open_with_asset_cache(
                package.as_ptr(),
                package.len() as u32,
                std::ptr::null(),
                0,
                cache.as_ptr(),
                cache.len() as u32,
                &raw const selection,
                &raw mut campaign_handle,
            ),
            MC_STATUS_OK,
            "{}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_eq!(campaign_handle & 1, 0);
        assert_ne!(campaign_handle, runtime_handle);

        let mut campaign_world = McCampaignWorldInfoV2::default();
        assert_eq!(
            mc_native_campaign_world_info(
                campaign_handle,
                &raw mut campaign_world,
                struct_size::<McCampaignWorldInfoV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            (
                campaign_world.external_level,
                campaign_world.internal_level,
                campaign_world.map_type,
            ),
            (2, 1, 0)
        );

        // The generic campaign read endpoint must fall through to the runtime
        // adapter, not resolve the simultaneous retained campaign.
        let mut runtime_world = McCampaignWorldInfoV2::default();
        assert_eq!(
            mc_native_campaign_world_info(
                runtime_handle,
                &raw mut runtime_world,
                struct_size::<McCampaignWorldInfoV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            (
                runtime_world.external_level,
                runtime_world.internal_level,
                runtime_world.map_type,
            ),
            (3, 2, 0)
        );

        let session_generation = opened_runtime["session"]["generation"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        let session_tick = opened_runtime["session"]["tick"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        let runtime_query = McPresentationFrameQueryV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McPresentationFrameQueryV2>(),
            generation: session_generation,
            tick: session_tick,
            presentation_revision: session_tick,
            viewport_width: 320,
            viewport_height: 200,
            screen_stride: 320,
            vga_resolution: 1,
            ..McPresentationFrameQueryV2::default()
        };
        let mut runtime_indices = vec![0_u8; LEVEL1_VIEW_PIXELS];
        let mut runtime_frame = McPresentationFrameInfoV2::default();
        assert_eq!(
            mc_native_campaign_presentation_frame_copy(
                runtime_handle,
                &raw const runtime_query,
                runtime_indices.as_mut_ptr(),
                runtime_indices.len() as u32,
                std::ptr::null_mut(),
                0,
                &raw mut runtime_frame,
                struct_size::<McPresentationFrameInfoV2>(),
            ),
            MC_STATUS_OK,
            "{}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_eq!(
            (runtime_frame.generation, runtime_frame.tick),
            (session_generation, session_tick)
        );

        let campaign_query = McPresentationFrameQueryV2 {
            generation: 0,
            tick: 0,
            presentation_revision: 0,
            ..runtime_query
        };
        let mut campaign_indices = vec![0_u8; LEVEL1_VIEW_PIXELS];
        let mut campaign_frame = McPresentationFrameInfoV2::default();
        assert_eq!(
            mc_native_campaign_presentation_frame_copy(
                campaign_handle,
                &raw const campaign_query,
                campaign_indices.as_mut_ptr(),
                campaign_indices.len() as u32,
                std::ptr::null_mut(),
                0,
                &raw mut campaign_frame,
                struct_size::<McPresentationFrameInfoV2>(),
            ),
            MC_STATUS_OK
        );
        assert_ne!(runtime_frame.index_hash, campaign_frame.index_hash);

        // Independent write endpoints retain their typed owners.
        let runtime_generation = opened_runtime["generation"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        assert_eq!(
            mc_game_runtime_pause_v1(runtime_handle, runtime_generation),
            MC_STATUS_OK
        );
        assert_eq!(identity(runtime_handle)["mode"], "paused");
        assert_eq!(
            mc_native_campaign_session_reset(campaign_handle),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_campaign_presentation_frame_copy(
                campaign_handle,
                &raw const campaign_query,
                campaign_indices.as_mut_ptr(),
                campaign_indices.len() as u32,
                std::ptr::null_mut(),
                0,
                &raw mut campaign_frame,
                struct_size::<McPresentationFrameInfoV2>(),
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(identity(runtime_handle)["mode"], "paused");

        assert_eq!(
            mc_native_campaign_session_destroy(campaign_handle),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_campaign_world_info(
                campaign_handle,
                &raw mut campaign_world,
                struct_size::<McCampaignWorldInfoV2>(),
            ),
            MC_STATUS_UNKNOWN_HANDLE
        );
        assert_eq!(identity(runtime_handle)["mode"], "paused");

        assert_eq!(mc_game_runtime_destroy_v1(runtime_handle), MC_STATUS_OK);
        let mut length = 0_u32;
        assert_eq!(
            mc_game_runtime_identity_json_v1(
                runtime_handle,
                std::ptr::null_mut(),
                0,
                &raw mut length,
            ),
            MC_STATUS_UNKNOWN_HANDLE
        );
        assert_eq!(
            mc_native_campaign_presentation_frame_copy(
                runtime_handle,
                &raw const runtime_query,
                runtime_indices.as_mut_ptr(),
                runtime_indices.len() as u32,
                std::ptr::null_mut(),
                0,
                &raw mut runtime_frame,
                struct_size::<McPresentationFrameInfoV2>(),
            ),
            MC_STATUS_UNKNOWN_HANDLE
        );
    }
}
