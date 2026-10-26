//! Internal campaign lifecycle adapters and frozen effect projection.
//!
//! Adapter selection remains static and adds no runtime registry or alternate
//! authority. CF-12 also projects the canonical descriptor-backed journal into
//! the existing public event layout without exposing Level-2 carrier types.

use std::{marker::PhantomData, path::Path, sync::Arc};

use sha2::{Digest, Sha256};
use sim_core::{
    FlightEvent, FlightTuning, PlayerCommand, SpellFireSource,
    campaign::journal::{CampaignEffect, EffectRecord},
    game_definition::{
        GameDefinition, Mc2OwnedInputs, SemanticId, SemanticValue, default_catalog, load_mc2_owned,
    },
    goat::GoatBehaviorConfig,
    native_world::{Level2SharedSpellEvent, NativeWorld},
};
use sim_session::{
    AudioAction, CompatibilityReceipt, EventPage, GameAdapter, GameAssetReference,
    GameAudioCommand, GameCommand, GameCompletionStatus, GameEvent, GameSessionBackend,
    LateralControl, LongitudinalControl, RuntimeActorId, RuntimeParticipantId,
};

use super::{
    AbiError, AbiResult, MC_ASSET_REF_NONE, MC_NATIVE_ABI_VERSION, MC_STATUS_INVALID_ARGUMENT,
    MC_STATUS_IO_ERROR, MC_STATUS_VALIDATION_FAILED, McEventV2, McSessionSnapshotV2,
    SessionDefinitionTuning, SessionRecord, struct_size, validation::load_terrain_frame_assets,
};

pub(super) const MC_CAMPAIGN_EFFECT_DISPATCH: u32 = 0x100;
pub(super) const MC_CAMPAIGN_EFFECT_SHARED_SPELL: u32 = 0x101;
pub(super) const MC_CAMPAIGN_EFFECT_FLIGHT: u32 = 0x102;
pub(super) const MC_CAMPAIGN_EFFECT_SCENE_CHANGE: u32 = 0x103;
pub(super) const MC_CAMPAIGN_EFFECT_OBJECTIVE_PRESENTATION: u32 = 0x104;

/// Project one canonical campaign effect into the already-frozen `McEventV2`
/// transport layout. The broad `kind` selects the effect domain; `code` and
/// the scalar payload remain a transport projection, never gameplay authority.
pub(super) fn campaign_effect_snapshot(
    generation: u64,
    state_hash: u64,
    record: &EffectRecord,
) -> McEventV2 {
    let mut row = McEventV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McEventV2>(),
        generation,
        sequence: record.sequence.wrapping_add(1),
        state_hash,
        player_index: -1,
        asset_ref_index: MC_ASSET_REF_NONE,
        ..McEventV2::default()
    };
    match &record.effect {
        CampaignEffect::Dispatch(event) => {
            let (pass, kind, family, flags, source_slot) = event.campaign_projection();
            row.kind = MC_CAMPAIGN_EFFECT_DISPATCH;
            row.code = u32::from(pass)
                | (u32::from(kind) << 8)
                | (u32::from(family) << 16)
                | (u32::from(flags) << 24);
            row.entity_index = source_slot;
            row.value_0 = i32::try_from(record.turn).unwrap_or(i32::MAX);
        }
        CampaignEffect::SharedSpell(event) => {
            row.kind = MC_CAMPAIGN_EFFECT_SHARED_SPELL;
            match *event {
                Level2SharedSpellEvent::SpellCollected {
                    source_slot,
                    player_entity_index,
                    family,
                } => {
                    row.code = 1;
                    row.entity_index = source_slot;
                    row.value_0 = i32::from(player_entity_index);
                    row.value_1 = i32::from(family);
                }
                Level2SharedSpellEvent::SpellAcquired {
                    source_slot,
                    player_entity_index,
                    family,
                    holder_action,
                } => {
                    row.code = 2;
                    row.entity_index = source_slot;
                    row.value_0 = i32::from(player_entity_index);
                    row.value_1 = i32::from(family) | (i32::from(holder_action) << 8);
                }
                Level2SharedSpellEvent::ShieldDamageApplied {
                    target_entity_index,
                    attacker_entity_index,
                    damage,
                } => {
                    row.code = 3 | (u32::from(attacker_entity_index) << 8);
                    row.entity_index = target_entity_index;
                    row.value_0 = damage.raw_damage;
                    row.value_1 = damage.applied_damage;
                }
                Level2SharedSpellEvent::ExperienceScrollCollected {
                    source_slot,
                    player_entity_index,
                } => {
                    row.code = 4;
                    row.entity_index = source_slot;
                    row.value_0 = i32::from(player_entity_index);
                }
                Level2SharedSpellEvent::AbilityImproved {
                    source_family,
                    tier,
                } => {
                    row.code = 5;
                    row.value_0 = i32::from(tier);
                    row.value_1 = i32::from(source_family);
                }
            }
        }
        CampaignEffect::Flight(event) => {
            row.kind = MC_CAMPAIGN_EFFECT_FLIGHT;
            match *event {
                FlightEvent::PrepareSound {
                    entity_index,
                    player_index,
                    sound_id,
                    ..
                } => {
                    row.code = 1;
                    row.entity_index = entity_index;
                    row.player_index = player_index;
                    row.value_0 = i32::from(sound_id);
                    if record.audio.is_some() {
                        row.asset_ref_index = u32::from(sound_id);
                    }
                }
                FlightEvent::EndLoop {
                    entity_index,
                    player_index,
                    sound_id,
                    ..
                } => {
                    row.code = 2;
                    row.entity_index = entity_index;
                    row.player_index = player_index;
                    row.value_0 = i32::from(sound_id);
                    if record.audio.is_some() {
                        row.asset_ref_index = u32::from(sound_id);
                    }
                }
                FlightEvent::UpdateMusic {
                    mode,
                    timer_multiplier,
                } => {
                    row.code = 3;
                    row.value_0 = i32::from(mode);
                    row.value_1 = i32::from(timer_multiplier);
                }
                FlightEvent::RequestSpellActivation {
                    source,
                    holder_entity_index,
                    direction_mask,
                } => {
                    row.code = 4;
                    row.entity_index = holder_entity_index;
                    row.value_0 = match source {
                        SpellFireSource::Left => 1,
                        SpellFireSource::Right => 2,
                        SpellFireSource::Selected => 3,
                    };
                    row.value_1 = i32::from(direction_mask);
                }
                FlightEvent::ShowHelpHint { hint_index, state } => {
                    row.code = 5;
                    row.value_0 = i32::from(hint_index);
                    row.value_1 = i32::from(state);
                }
            }
        }
        CampaignEffect::SceneChange { scene } => {
            row.kind = MC_CAMPAIGN_EFFECT_SCENE_CHANGE;
            row.code = 1;
            row.value_0 = i32::from(*scene);
            row.value_1 = i32::try_from(record.turn).unwrap_or(i32::MAX);
        }
        CampaignEffect::ObjectivePresentation(actions) => {
            row.kind = MC_CAMPAIGN_EFFECT_OBJECTIVE_PRESENTATION;
            row.code = u32::from(actions.palette_flash) | (u32::from(actions.duck_audio) << 1);
            row.entity_index = u16::from(actions.textbox_ticks.unwrap_or(0));
            if let Some(speech) = actions.speech {
                row.code |= u32::from(speech.track) << 8 | u32::from(speech.segment) << 16;
                row.value_0 = speech.start_ms as i32;
                row.value_1 = speech.length_ms as i32;
            }
        }
    }
    row
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct InternalCampaignKey {
    pub(super) external_level: u16,
    pub(super) internal_level: u16,
    pub(super) map_type: u8,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct InternalCampaignOpen<'a> {
    pub(super) package_dir: &'a Path,
    pub(super) closure_path: Option<&'a Path>,
    pub(super) owner_cache: Option<&'a Path>,
}

pub(super) trait InternalCampaignAdapter {
    type Session;
    type StepInput;
    type Snapshot;

    const KEY: InternalCampaignKey;

    fn open(key: InternalCampaignKey, input: InternalCampaignOpen<'_>) -> AbiResult<Self::Session>;
    fn step(session: &mut Self::Session, input: Self::StepInput) -> AbiResult<Self::Snapshot>;
    fn reset(session: &mut Self::Session) -> AbiResult<Self::Snapshot>;
}

pub(super) struct CampaignFacade<A>(PhantomData<fn() -> A>);

impl<A: InternalCampaignAdapter> CampaignFacade<A> {
    pub(super) fn open(
        key: InternalCampaignKey,
        input: InternalCampaignOpen<'_>,
    ) -> AbiResult<A::Session> {
        A::open(key, input)
    }

    pub(super) fn step(session: &mut A::Session, input: A::StepInput) -> AbiResult<A::Snapshot> {
        A::step(session, input)
    }

    pub(super) fn reset(session: &mut A::Session) -> AbiResult<A::Snapshot> {
        A::reset(session)
    }
}

pub(super) struct SealedLevel1Adapter;

#[derive(Debug)]
pub(super) struct SealedLevel1Backend {
    session: SessionRecord,
    package_base: GameDefinition,
    package_graph_hash: String,
    artifact_graph_hash: String,
}

impl SealedLevel1Adapter {
    fn integer_value(definition: &GameDefinition, id_text: &str) -> AbiResult<i64> {
        let id = SemanticId::parse(id_text).map_err(|error| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!("invalid built-in integer semantic ID: {error}"),
            )
        })?;
        let SemanticValue::Integer(value) = definition.value(&id).ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!("definition has no {id_text} value"),
            )
        })?
        else {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!("{id_text} is not an integer"),
            ));
        };
        Ok(*value)
    }

    fn enum_value<'a>(definition: &'a GameDefinition, id_text: &str) -> AbiResult<&'a str> {
        let id = SemanticId::parse(id_text).map_err(|error| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!("invalid built-in enum semantic ID: {error}"),
            )
        })?;
        let Some(SemanticValue::Enum(value)) = definition.value(&id) else {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!("{id_text} is not an enum"),
            ));
        };
        Ok(value)
    }

    fn definition_tuning(definition: &GameDefinition) -> AbiResult<SessionDefinitionTuning> {
        let max_forward_speed = i16::try_from(Self::integer_value(
            definition,
            "flight.player.max_forward_speed",
        )?)
        .map_err(|_| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "flight.player.max_forward_speed exceeds the Rust flight carrier",
            )
        })?;
        let forward_acceleration = i16::try_from(Self::integer_value(
            definition,
            "flight.player.forward_acceleration",
        )?)
        .map_err(|_| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "flight.player.forward_acceleration exceeds the Rust flight carrier",
            )
        })?;
        let mana_cost = i32::try_from(Self::integer_value(definition, "spell.fireball.mana_cost")?)
            .map_err(|_| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "spell.fireball.mana_cost exceeds the Rust mana carrier",
                )
            })?;
        let mut goat_config = GoatBehaviorConfig::LEVEL_ONE;
        match Self::enum_value(definition, "ai.enemy.default.policy")? {
            "classic_goat_v1" => {}
            "vigilant_goat_v1" => {
                goat_config.perception_cadence = 1;
                goat_config.perception_radius = u16::MAX;
                goat_config.perception_half_angle = 1_025;
                // Owner row 98 uses bits 0 and 3. Engine-owned bit 4 selects
                // the bounded vigilant policy's always-on perception pass;
                // the default policy remains the exact owner configuration.
                goat_config.behavior_flags |= 0x10;
            }
            value => {
                return Err(AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    format!("unsupported compiled Goat policy {value}"),
                ));
            }
        }
        Ok(SessionDefinitionTuning {
            flight: FlightTuning {
                max_forward_speed,
                forward_acceleration,
            },
            fireball_mana_cost: mana_cost,
            goat_config,
        })
    }

    fn validate_key(key: InternalCampaignKey) -> AbiResult<()> {
        if key == Self::KEY {
            Ok(())
        } else {
            Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!(
                    "unknown internal campaign key {}/{}/{}",
                    key.external_level, key.internal_level, key.map_type
                ),
            ))
        }
    }

    fn load_world(input: InternalCampaignOpen<'_>) -> AbiResult<NativeWorld> {
        let closure_path = input.closure_path.ok_or_else(|| {
            AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "sealed Level 1 requires a closure path",
            )
        })?;
        let result = if let Some(owner_cache) = input.owner_cache {
            NativeWorld::load_with_owner_cache(input.package_dir, closure_path, owner_cache)
        } else {
            NativeWorld::load(input.package_dir, closure_path)
        };
        result.map_err(|error| {
            let message = error.to_string();
            let status = if message.starts_with("cannot read ") {
                MC_STATUS_IO_ERROR
            } else {
                MC_STATUS_VALIDATION_FAILED
            };
            AbiError::new(status, message)
        })
    }
}

impl GameAdapter for SealedLevel1Adapter {
    type Backend = SealedLevel1Backend;
    type OpenInput = Mc2OwnedInputs;
    type Error = AbiError;

    fn open_backend(
        &self,
        definition: &GameDefinition,
        input: Self::OpenInput,
    ) -> Result<Self::Backend, Self::Error> {
        let catalog = default_catalog().map_err(|error| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!("cannot construct neutral semantic catalog: {error}"),
            )
        })?;
        let _validated_owner_provider = load_mc2_owned(&input, &catalog).map_err(|error| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!("cannot validate MC2-owned definition inputs: {error}"),
            )
        })?;
        // The owner inputs provide this compatibility backend's world data;
        // the neutral immutable definition may come from any validated package
        // requiring the same compiled capabilities. No package ID is privileged.
        let definition_tuning = Self::definition_tuning(definition)?;
        let world = NativeWorld::load_with_owner_cache(
            &input.export_root,
            &input.closure_path,
            &input.cache_root,
        )
        .map_err(|error| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!("cannot open neutral MC2-owned backend: {error}"),
            )
        })?;
        let terrain_frame_assets = load_terrain_frame_assets(&input.cache_root)?;
        Ok(SealedLevel1Backend {
            session: SessionRecord::new_with_terrain_frame_assets_and_definition_tuning(
                world,
                Some(Arc::new(terrain_frame_assets)),
                definition_tuning,
            )?,
            package_base: definition.clone(),
            package_graph_hash: definition.package_graph_hash().to_owned(),
            artifact_graph_hash: definition.artifact_graph_hash().to_owned(),
        })
    }
}

impl GameSessionBackend for SealedLevel1Backend {
    type Command = GameCommand;
    type Snapshot = sim_session::GameSnapshot;
    type Error = AbiError;

    fn tick(&self) -> u64 {
        self.session.state.tick
    }

    fn completion_status(&self) -> GameCompletionStatus {
        GameCompletionStatus::Unsupported
    }

    fn snapshot(&self) -> Self::Snapshot {
        self.session.game_snapshot()
    }

    fn events_after(&self, cursor: u64, limit: usize) -> Result<EventPage, Self::Error> {
        if cursor > self.session.event_head {
            return Err(AbiError::new(
                super::MC_STATUS_BOUNDS,
                "event cursor is beyond current event head",
            ));
        }
        if let Some(oldest) = self.session.events.front()
            && cursor.saturating_add(1) < oldest.sequence
        {
            return Err(AbiError::new(
                super::MC_STATUS_CURSOR_EXPIRED,
                "event cursor predates the retained journal",
            ));
        }
        let available = self
            .session
            .events
            .iter()
            .filter(|event| event.sequence > cursor)
            .collect::<Vec<_>>();
        let count = available.len().min(limit);
        let compatibility = self.compatibility_receipt(&available[..count]);
        let events = available
            .iter()
            .take(count)
            .map(|event| self.game_event(event))
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = events.last().map_or(cursor, |event| event.sequence);
        Ok(EventPage {
            requested_cursor: cursor,
            next_cursor,
            event_head: self.session.event_head,
            has_more: count < available.len(),
            events,
            compatibility: Some(compatibility),
        })
    }

    fn audio_for_event(&self, event_sequence: u64) -> Result<GameAudioCommand, Self::Error> {
        let command = self
            .session
            .audio_commands
            .get(&event_sequence)
            .ok_or_else(|| {
                AbiError::new(
                    super::MC_STATUS_BOUNDS,
                    format!("event sequence {event_sequence} has no positional audio command"),
                )
            })?;
        let asset = self
            .asset_reference(command.asset_ref_index)?
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "audio command has no package asset identity",
                )
            })?;
        let action = match command.action {
            0 => AudioAction::Prepared,
            1 => AudioAction::Start,
            2 => AudioAction::Restart,
            3 => AudioAction::SuppressExisting,
            4 => AudioAction::SuppressNoVoice,
            5 => AudioAction::SuppressDistanceOrVolume,
            6 => AudioAction::SuppressPendingReplaced,
            7 => AudioAction::UpdateExisting,
            8 => AudioAction::EndLoop,
            9 => AudioAction::SuppressNoExisting,
            value => {
                return Err(AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    format!("unknown adapter-private audio action {value}"),
                ));
            }
        };
        Ok(GameAudioCommand {
            event_sequence: command.event_sequence,
            state_hash: command.state_hash,
            asset,
            relative_position: [command.relative_x, command.relative_y, command.relative_z],
            playback_rate_hz: command.playback_rate_hz,
            actor: (command.entity_index != 0).then(|| {
                RuntimeActorId(
                    self.opaque_runtime_handle(b"actor", u64::from(command.entity_index)),
                )
            }),
            participant: (command.player_index >= 0).then_some(RuntimeParticipantId(
                self.opaque_runtime_handle(
                    b"participant",
                    command.player_index.cast_unsigned().into(),
                ),
            )),
            sound_flags: command.sound_flags,
            volume_q15: command.volume_q15,
            pan_q16: command.pan_q16,
            rate_delta_percent: command.rate_delta_percent,
            voice_index: command.voice_index,
            play_type: command.play_type,
            loop_count: command.loop_count,
            action,
            pooled_flush_rank: command.pooled_flush_rank,
        })
    }

    fn step(&mut self, command: Self::Command) -> Result<(), Self::Error> {
        if command.abandon_action.is_some() {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "source menu actions are unavailable in this session",
            ));
        }
        self.session.step(mc2_player_command(command));
        Ok(())
    }

    fn paused_frame(&mut self) -> Result<(), Self::Error> {
        // The sealed compatibility session is not owned by GameRuntime. Its
        // legacy pause transport remains the sole caller of that route.
        Ok(())
    }

    fn reset_for_definition(&mut self, definition: &GameDefinition) -> Result<(), Self::Error> {
        if definition.package_graph_hash() != self.package_graph_hash
            || definition.artifact_graph_hash() != self.artifact_graph_hash
        {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "session reset cannot substitute a package or artifact graph; reopen resources first",
            ));
        }
        let catalog = default_catalog().map_err(|error| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!("cannot construct neutral semantic catalog: {error}"),
            )
        })?;
        definition
            .verify_semantic_variant_of(&self.package_base, &catalog)
            .map_err(|error| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    format!("rebound definition is not a validated package variant: {error}"),
                )
            })?;
        let definition_tuning = SealedLevel1Adapter::definition_tuning(definition)?;
        self.session.reset_with_definition_tuning(definition_tuning);
        Ok(())
    }
}

fn mc2_player_command(command: GameCommand) -> PlayerCommand {
    let mut flags = match command.longitudinal {
        LongitudinalControl::Coast => 0,
        LongitudinalControl::Accelerate => 0x01,
        LongitudinalControl::Decelerate => 0x02,
    };
    flags |= match command.lateral {
        LateralControl::Neutral => 0,
        LateralControl::Negative => 0x04,
        LateralControl::Positive => 0x08,
    };
    flags |= u8::from(command.cast_left) * 0x10;
    flags |= u8::from(command.cast_right) * 0x20;
    flags |= u8::from(command.cast_selected) * 0x40;
    PlayerCommand {
        opcode: if command.recenter || command.full_stop {
            0x27
        } else {
            0
        },
        argument_1: i8::from(command.full_stop),
        roll: command.roll_axis,
        pitch: command.pitch_axis,
        flags: flags.cast_signed(),
        argument_6: command.look_yaw.cast_unsigned(),
        argument_8: command.look_pitch.cast_unsigned(),
        ..PlayerCommand::default()
    }
}

impl SealedLevel1Backend {
    pub(super) fn projection(&self) -> &SessionRecord {
        &self.session
    }

    pub(super) fn projection_mut(&mut self) -> &mut SessionRecord {
        &mut self.session
    }

    fn opaque_runtime_handle(&self, namespace: &[u8], source_value: u64) -> u64 {
        let mut digest = Sha256::new();
        digest.update(b"mc2-owned-runtime-handle/v1\0");
        digest.update(self.artifact_graph_hash.as_bytes());
        digest.update([0]);
        digest.update(namespace);
        digest.update([0]);
        digest.update(source_value.to_le_bytes());
        let bytes: [u8; 8] = digest.finalize()[..8]
            .try_into()
            .expect("SHA-256 prefix is eight bytes");
        u64::from_le_bytes(bytes).max(1)
    }

    fn compatibility_receipt(&self, events: &[&super::McEventV2]) -> CompatibilityReceipt {
        let mut event_content = Sha256::new();
        let mut event_lifecycle = Sha256::new();
        let mut audio_content = Sha256::new();
        let mut audio_lifecycle = Sha256::new();
        let mut audio_count = 0_u32;
        for event in events {
            event_lifecycle.update(event.generation.to_le_bytes());
            for digest in [&mut event_content, &mut event_lifecycle] {
                digest.update(event.sequence.to_le_bytes());
                digest.update(event.state_hash.to_le_bytes());
                digest.update(event.kind.to_le_bytes());
                digest.update(event.code.to_le_bytes());
                digest.update(event.entity_index.to_le_bytes());
                digest.update(event.player_index.to_le_bytes());
                digest.update(event.value_0.to_le_bytes());
                digest.update(event.value_1.to_le_bytes());
                digest.update(event.asset_ref_index.to_le_bytes());
            }
            let Some(audio) = self.session.audio_commands.get(&event.sequence) else {
                continue;
            };
            audio_count = audio_count
                .checked_add(1)
                .expect("bounded audio count fits u32");
            audio_lifecycle.update(audio.generation.to_le_bytes());
            for digest in [&mut audio_content, &mut audio_lifecycle] {
                digest.update(audio.event_sequence.to_le_bytes());
                digest.update(audio.state_hash.to_le_bytes());
                digest.update(audio.asset_ref_index.to_le_bytes());
                digest.update(audio.relative_x.to_le_bytes());
                digest.update(audio.relative_y.to_le_bytes());
                digest.update(audio.relative_z.to_le_bytes());
                digest.update(audio.playback_rate_hz.to_le_bytes());
                digest.update(audio.sound_id.to_le_bytes());
                digest.update(audio.entity_index.to_le_bytes());
                digest.update(audio.player_index.to_le_bytes());
                digest.update(audio.sound_flags.to_le_bytes());
                digest.update(audio.volume_q15.to_le_bytes());
                digest.update(audio.pan_q16.to_le_bytes());
                digest.update(audio.rate_delta_percent.to_le_bytes());
                digest.update(audio.voice_index.to_le_bytes());
                digest.update([audio.play_type]);
                digest.update(audio.loop_count.to_le_bytes());
                digest.update([audio.action, audio.pooled_flush_rank]);
                let pooled_flush_id =
                    u64::from(audio.reserved[0]) | (u64::from(audio.reserved[1]) << 32);
                digest.update(pooled_flush_id.to_le_bytes());
            }
        }
        CompatibilityReceipt {
            contract: "mc2-owned-event-audio-identity/v1".to_owned(),
            event_count: u32::try_from(events.len()).expect("bounded event page fits u32"),
            event_content_sha256: format!("{:x}", event_content.finalize()),
            event_lifecycle_sha256: format!("{:x}", event_lifecycle.finalize()),
            audio_count,
            audio_content_sha256: format!("{:x}", audio_content.finalize()),
            audio_lifecycle_sha256: format!("{:x}", audio_lifecycle.finalize()),
        }
    }

    fn asset_reference(&self, index: u32) -> Result<Option<GameAssetReference>, AbiError> {
        if index == super::MC_ASSET_REF_NONE {
            return Ok(None);
        }
        let asset = self
            .session
            .world
            .asset_refs()
            .get(usize::try_from(index).map_err(|_| {
                AbiError::new(
                    super::MC_STATUS_BOUNDS,
                    "asset reference index exceeds usize",
                )
            })?)
            .ok_or_else(|| {
                AbiError::new(
                    super::MC_STATUS_BOUNDS,
                    format!("asset reference index {index} is out of bounds"),
                )
            })?;
        Ok(Some(GameAssetReference {
            id: asset.asset_id.clone(),
            content_address: asset.content_address.clone(),
        }))
    }

    fn game_event(&self, event: &super::McEventV2) -> Result<GameEvent, AbiError> {
        const EVENT_IDS: [&str; 39] = [
            "flight.movement_resolved",
            "audio.sound_prepared",
            "audio.loop_ended",
            "audio.music_updated",
            "spell.activation_requested",
            "ui.help_hint_requested",
            "audio.music_started",
            "creature.sound_requested",
            "resource.mana_converted",
            "resource.mana_sphere_requested",
            "actor.removal_queued",
            "actor.reclaimed",
            "ai.engagement_boundary",
            "objective.advanced",
            "spell.collected",
            "combat.fireball_spawned",
            "combat.fireball_impacted",
            "combat.actor_damaged",
            "combat.actor_killed",
            "economy.castle_built",
            "economy.castle_mana_changed",
            "combat.player_damaged",
            "combat.player_died",
            "session.completed",
            "combat.archer_arrow_spawned",
            "audio.archer_arrow_requested",
            "combat.firefly_fireball_spawned",
            "audio.firefly_charge_requested",
            "combat.castle_projectile_spawned",
            "economy.castle_placement_rejected",
            "audio.castle_construction_requested",
            "spell.possession_spawned",
            "resource.mana_sphere_spawned",
            "resource.mana_sphere_claimed",
            "resource.mana_sphere_collected",
            "session.exit_started",
            "session.exit_completed",
            "world.water_splashed",
            "ability.improved",
        ];
        let event_id =
            EVENT_IDS
                .get(usize::try_from(event.kind).map_err(|_| {
                    AbiError::new(super::MC_STATUS_BOUNDS, "event kind exceeds usize")
                })?)
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        format!("unknown adapter-private event kind {}", event.kind),
                    )
                })?;
        Ok(GameEvent {
            sequence: event.sequence,
            state_hash: event.state_hash,
            event_id: sim_core::game_definition::SemanticId::parse(event_id).map_err(|error| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    format!("invalid canonical event ID: {error}"),
                )
            })?,
            actor: (event.entity_index != 0).then(|| {
                RuntimeActorId(self.opaque_runtime_handle(b"actor", u64::from(event.entity_index)))
            }),
            participant: (event.player_index >= 0).then_some(RuntimeParticipantId(
                self.opaque_runtime_handle(
                    b"participant",
                    event.player_index.cast_unsigned().into(),
                ),
            )),
            asset: self.asset_reference(event.asset_ref_index)?,
        })
    }
}

impl InternalCampaignAdapter for SealedLevel1Adapter {
    type Session = SessionRecord;
    type StepInput = PlayerCommand;
    type Snapshot = McSessionSnapshotV2;

    const KEY: InternalCampaignKey = InternalCampaignKey {
        external_level: 1,
        internal_level: 0,
        map_type: 1,
    };

    fn open(key: InternalCampaignKey, input: InternalCampaignOpen<'_>) -> AbiResult<Self::Session> {
        Self::validate_key(key)?;
        SessionRecord::new(Self::load_world(input)?)
    }

    fn step(session: &mut Self::Session, input: Self::StepInput) -> AbiResult<Self::Snapshot> {
        session.step(input);
        Ok(session.snapshot())
    }

    fn reset(session: &mut Self::Session) -> AbiResult<Self::Snapshot> {
        session.reset();
        Ok(session.snapshot())
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};

    use sim_core::game_definition::{
        ChangeSetBuilder, IntegerParameter, OptionParameter, PublicationFault, RequestedTiming,
        WorkspaceStore,
    };
    use sim_core::game_session::{GameSession, SessionGuard};

    use super::*;

    fn fixture_paths() -> (PathBuf, PathBuf, PathBuf) {
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        (
            private.join("generated/level1-export-m"),
            private.join("generated/level1-asset-closure.json"),
            private.join("generated/level1-unity-asset-cache-plan29-final"),
        )
    }

    fn open_fixture() -> SessionRecord {
        let (package_dir, closure_path, owner_cache) = fixture_paths();
        CampaignFacade::<SealedLevel1Adapter>::open(
            SealedLevel1Adapter::KEY,
            InternalCampaignOpen {
                package_dir: &package_dir,
                closure_path: Some(&closure_path),
                owner_cache: Some(&owner_cache),
            },
        )
        .expect("sealed Level 1 opens through the internal campaign facade")
    }

    fn guard(session: &GameSession<SealedLevel1Backend>) -> SessionGuard {
        let identity = session.identity();
        SessionGuard {
            session_id: identity.session_id,
            generation: identity.generation,
            expected_tick: identity.tick,
        }
    }

    #[test]
    fn neutral_flight_command_maps_inside_the_mc2_owned_adapter() {
        let command = GameCommand {
            show_objective: false,
            assign_spell: None,
            mark_spell: None,
            cycle_marked_spell: None,
            abandon_action: None,
            longitudinal: LongitudinalControl::Decelerate,
            lateral: LateralControl::Positive,
            roll_axis: -32,
            pitch_axis: 24,
            cast_left: true,
            cast_right: false,
            cast_selected: true,
            recover_player: false,
            remove_castle_stage: false,
            elapsed_centiseconds: 0,
            select_spell: None,
            recenter: false,
            full_stop: false,
            look_yaw: -17,
            look_pitch: 23,
        };
        let mapped = mc2_player_command(command.clone());
        assert_eq!(mapped.opcode, 0);
        assert_eq!(mapped.argument_1, 0);
        assert_eq!(mapped.argument_2, 0);
        assert_eq!(mapped.flags.cast_unsigned(), 0x02 | 0x08 | 0x10 | 0x40);
        assert_eq!(mapped.roll, -32);
        assert_eq!(mapped.pitch, 24);
        assert_eq!(mapped.argument_6.cast_signed(), -17);
        assert_eq!(mapped.argument_8.cast_signed(), 23);

        for (recenter, full_stop, argument_1) in
            [(true, false, 0), (false, true, 1), (true, true, 1)]
        {
            let recentered = mc2_player_command(GameCommand {
                recenter,
                full_stop,
                ..command.clone()
            });
            assert_eq!(
                recentered,
                PlayerCommand {
                    opcode: 0x27,
                    argument_1,
                    ..mapped
                },
                "recenter={recenter}, full_stop={full_stop}: preserve movement, cast and look fields"
            );
        }
        assert_eq!(
            mc2_player_command(GameCommand::default()),
            PlayerCommand::default()
        );
    }

    #[test]
    fn neutral_event_page_rejects_an_expired_cursor_without_changing_the_journal() {
        let mut session = open_fixture();
        let (export, closure, cache) = fixture_paths();
        let package_base = load_mc2_owned(
            &Mc2OwnedInputs::new(export, closure, cache),
            &default_catalog().unwrap(),
        )
        .unwrap();
        session.events.clear();
        session.event_head = 4;
        session.events.push_back(super::super::McEventV2 {
            generation: session.generation,
            sequence: 3,
            kind: super::super::MC_EVENT_MOVEMENT_RESOLUTION,
            ..super::super::McEventV2::default()
        });
        session.events.push_back(super::super::McEventV2 {
            generation: session.generation,
            sequence: 4,
            kind: super::super::MC_EVENT_MOVEMENT_RESOLUTION,
            ..super::super::McEventV2::default()
        });
        let before = session.events.clone();
        let backend = SealedLevel1Backend {
            session,
            package_base,
            package_graph_hash: "test-package".to_owned(),
            artifact_graph_hash: "test-artifact".to_owned(),
        };
        assert_ne!(backend.opaque_runtime_handle(b"actor", 1), 1);
        assert_ne!(
            backend.opaque_runtime_handle(b"actor", 1),
            backend.opaque_runtime_handle(b"participant", 1)
        );
        let error = backend.events_after(0, 16).unwrap_err();
        assert_eq!(error.status, super::super::MC_STATUS_CURSOR_EXPIRED);
        assert_eq!(backend.session.events, before);
        assert_eq!(backend.session.event_head, 4);
    }

    #[test]
    fn neutral_mc2_owned_session_matches_legacy_step_and_reset_in_the_same_build() {
        let (export, closure, cache) = fixture_paths();
        let inputs = Mc2OwnedInputs::new(export, closure, cache);
        let definition = Arc::new(
            load_mc2_owned(&inputs, &default_catalog().unwrap())
                .expect("real owner inputs materialize the neutral definition"),
        );
        let mut neutral = GameSession::open(&SealedLevel1Adapter, definition, 41, inputs).unwrap();
        let mut legacy = open_fixture();
        assert_eq!(neutral.snapshot().value, legacy.game_snapshot());

        let command = GameCommand {
            longitudinal: LongitudinalControl::Accelerate,
            ..GameCommand::default()
        };
        let neutral_step = neutral.step(guard(&neutral), command.clone()).unwrap();
        let legacy_step =
            CampaignFacade::<SealedLevel1Adapter>::step(&mut legacy, mc2_player_command(command))
                .unwrap();
        assert_eq!(neutral_step.value, legacy.game_snapshot());
        assert_eq!(legacy_step, legacy.snapshot());
        assert_eq!(neutral_step.identity.tick, legacy_step.tick);
        assert_eq!(neutral_step.identity.generation, legacy_step.generation);
        let event_page = neutral
            .events_after(guard(&neutral), 0, usize::MAX)
            .unwrap();
        assert_eq!(event_page.value.event_head, legacy_step.event_head);
        assert_eq!(event_page.value.next_cursor, legacy_step.event_head);
        assert!(!event_page.value.has_more);
        assert!(!event_page.value.events.is_empty());
        assert!(event_page.value.events.iter().all(|event| {
            let id = event.event_id.as_str();
            !id.contains("level1") && !id.contains("source")
        }));

        let neutral_reset = neutral.reset(guard(&neutral)).unwrap();
        let legacy_reset = CampaignFacade::<SealedLevel1Adapter>::reset(&mut legacy).unwrap();
        assert_eq!(neutral_reset.value, legacy.game_snapshot());
        assert_eq!(legacy_reset, legacy.snapshot());
        assert_eq!(neutral_reset.identity.tick, 0);
        assert_eq!(neutral_reset.identity.generation, 2);
        assert_eq!(legacy_reset.generation, 2);
    }

    #[test]
    fn real_owner_sessions_rebind_only_the_explicitly_named_session() {
        let (export, closure, cache) = fixture_paths();
        let inputs = Mc2OwnedInputs::new(export, closure, cache);
        let catalog = default_catalog().unwrap();
        let original = load_mc2_owned(&inputs, &catalog).unwrap();
        let root = std::env::temp_dir().join(format!(
            "carpet-flight-f4-real-owner-binding-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let store = WorkspaceStore::create(&root, "real-owner-binding", original.clone()).unwrap();
        let mut first = GameSession::open(
            &SealedLevel1Adapter,
            Arc::new(original.clone()),
            51,
            inputs.clone(),
        )
        .unwrap();
        let mut second =
            GameSession::open(&SealedLevel1Adapter, Arc::new(original.clone()), 52, inputs)
                .unwrap();
        let second_before = second.snapshot();
        let change_set = ChangeSetBuilder::new(
            "real-owner-binding",
            &original,
            "publish-owner-speed",
            RequestedTiming::SessionReset,
        )
        .set_integer(IntegerParameter::PlayerMaxForwardSpeed, 16)
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
        let published = Arc::new(store.active_definition().unwrap());

        assert_eq!(first.identity().definition_hash, original.definition_hash());
        assert_eq!(second.snapshot(), second_before);
        let reopened = first.reopen(guard(&first), Arc::clone(&published)).unwrap();
        assert_eq!(
            reopened.identity.definition_hash,
            published.definition_hash()
        );
        assert_eq!(reopened.identity.definition_revision, 2);
        assert_eq!(reopened.identity.generation, 2);
        assert_eq!(second.snapshot(), second_before);
        assert_eq!(
            second.identity().definition_hash,
            original.definition_hash()
        );
        assert_eq!(second.identity().generation, 1);

        let accelerate = GameCommand {
            longitudinal: LongitudinalControl::Accelerate,
            ..GameCommand::default()
        };
        for _ in 0..2 {
            first.step(guard(&first), accelerate.clone()).unwrap();
            second.step(guard(&second), accelerate.clone()).unwrap();
        }
        let authored = first.snapshot().value;
        let baseline = second.snapshot().value;
        assert_eq!(authored.tick, baseline.tick);
        assert_eq!(authored.player_motion.target_speed, 16);
        assert_eq!(baseline.player_motion.target_speed, 32);
        assert_ne!(
            authored.player_motion.actual_speed,
            baseline.player_motion.actual_speed
        );
        assert_ne!(authored.player_pose, baseline.player_pose);
        assert_eq!(authored.caster_mana, baseline.caster_mana);
        assert_eq!(authored.objective_count, baseline.objective_count);
        assert_eq!(authored.action_sequence, baseline.action_sequence);
        assert_eq!(
            first.backend().projection().mission.global_rng(),
            second.backend().projection().mission.global_rng()
        );
        let authored_events = first
            .events_after(guard(&first), 0, 1_024)
            .unwrap()
            .value
            .events
            .into_iter()
            .map(|event| event.event_id)
            .collect::<Vec<_>>();
        let baseline_events = second
            .events_after(guard(&second), 0, 1_024)
            .unwrap()
            .value
            .events
            .into_iter()
            .map(|event| event.event_id)
            .collect::<Vec<_>>();
        assert_eq!(authored_events, baseline_events);

        first.reset(guard(&first)).unwrap();
        for _ in 0..2 {
            first.step(guard(&first), accelerate.clone()).unwrap();
        }
        assert_eq!(first.snapshot().value, authored);

        store
            .rollback(
                &applied.resulting_definition_hash,
                applied.resulting_definition_revision,
                &applied.transaction_id,
                "restore-owner-speed",
                PublicationFault::None,
            )
            .unwrap();
        let restored = Arc::new(store.active_definition().unwrap());
        first.reopen(guard(&first), restored).unwrap();
        second.reset(guard(&second)).unwrap();
        for _ in 0..2 {
            first.step(guard(&first), accelerate.clone()).unwrap();
            second.step(guard(&second), accelerate.clone()).unwrap();
        }
        assert_eq!(first.snapshot().value, second.snapshot().value);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn real_owner_fireball_cost_rebinds_the_ordinary_cast_without_projectile_drift() {
        let (export, closure, cache) = fixture_paths();
        let inputs = Mc2OwnedInputs::new(export, closure, cache);
        let catalog = default_catalog().unwrap();
        let original = load_mc2_owned(&inputs, &catalog).unwrap();
        let root = std::env::temp_dir().join(format!(
            "carpet-flight-f7-real-owner-fireball-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let store = WorkspaceStore::create(&root, "real-owner-fireball", original.clone()).unwrap();
        let change_set = ChangeSetBuilder::new(
            "real-owner-fireball",
            &original,
            "publish-owner-fireball-cost",
            RequestedTiming::SessionReset,
        )
        .set_integer(IntegerParameter::FireballManaCost, 200)
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
        let published = Arc::new(store.active_definition().unwrap());
        let mut baseline = GameSession::open(
            &SealedLevel1Adapter,
            Arc::new(original.clone()),
            61,
            inputs.clone(),
        )
        .unwrap();
        let mut authored =
            GameSession::open(&SealedLevel1Adapter, Arc::clone(&published), 62, inputs).unwrap();
        let cast = GameCommand {
            cast_left: true,
            ..GameCommand::default()
        };

        baseline.step(guard(&baseline), cast.clone()).unwrap();
        authored.step(guard(&authored), cast.clone()).unwrap();
        let baseline_snapshot = baseline.snapshot().value;
        let authored_snapshot = authored.snapshot().value;
        assert_eq!(baseline_snapshot.caster_mana, 900);
        assert_eq!(authored_snapshot.caster_mana, 800);
        assert_eq!(authored_snapshot.tick, baseline_snapshot.tick);
        assert_eq!(authored_snapshot.player_pose, baseline_snapshot.player_pose);
        assert_eq!(
            authored_snapshot.player_motion,
            baseline_snapshot.player_motion
        );
        assert_eq!(
            authored_snapshot.objective_count,
            baseline_snapshot.objective_count
        );
        assert_eq!(
            authored_snapshot.action_sequence,
            baseline_snapshot.action_sequence
        );
        assert_eq!(
            authored.backend().projection().mission.projectiles(),
            baseline.backend().projection().mission.projectiles()
        );
        assert_eq!(
            authored.backend().projection().mission.global_rng(),
            baseline.backend().projection().mission.global_rng()
        );
        let authored_events = authored
            .events_after(guard(&authored), 0, 1_024)
            .unwrap()
            .value
            .events
            .into_iter()
            .map(|event| event.event_id)
            .collect::<Vec<_>>();
        let baseline_events = baseline
            .events_after(guard(&baseline), 0, 1_024)
            .unwrap()
            .value
            .events
            .into_iter()
            .map(|event| event.event_id)
            .collect::<Vec<_>>();
        assert_eq!(authored_events, baseline_events);
        assert!(
            authored_events
                .iter()
                .any(|id| id.as_str() == "combat.fireball_spawned")
        );

        authored.reset(guard(&authored)).unwrap();
        authored.step(guard(&authored), cast.clone()).unwrap();
        assert_eq!(authored.snapshot().value, authored_snapshot);

        store
            .rollback(
                &applied.resulting_definition_hash,
                applied.resulting_definition_revision,
                &applied.transaction_id,
                "restore-owner-fireball-cost",
                PublicationFault::None,
            )
            .unwrap();
        authored
            .reopen(
                guard(&authored),
                Arc::new(store.active_definition().unwrap()),
            )
            .unwrap();
        baseline.reset(guard(&baseline)).unwrap();
        authored.step(guard(&authored), cast.clone()).unwrap();
        baseline.step(guard(&baseline), cast).unwrap();
        assert_eq!(authored.snapshot().value, baseline.snapshot().value);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn real_owner_vigilant_goat_policy_changes_the_ordinary_engagement_decision() {
        let (export, closure, cache) = fixture_paths();
        let inputs = Mc2OwnedInputs::new(export, closure, cache);
        let catalog = default_catalog().unwrap();
        let original = load_mc2_owned(&inputs, &catalog).unwrap();
        let root = std::env::temp_dir().join(format!(
            "carpet-flight-f8-real-owner-goat-policy-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let store =
            WorkspaceStore::create(&root, "real-owner-goat-policy", original.clone()).unwrap();
        let change_set = ChangeSetBuilder::new(
            "real-owner-goat-policy",
            &original,
            "publish-vigilant-goat-policy",
            RequestedTiming::SessionReset,
        )
        .select_option(OptionParameter::EnemyDefaultPolicy, "vigilant_goat_v1")
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
        let mut baseline = GameSession::open(
            &SealedLevel1Adapter,
            Arc::new(original.clone()),
            71,
            inputs.clone(),
        )
        .unwrap();
        let mut authored = GameSession::open(
            &SealedLevel1Adapter,
            Arc::new(store.active_definition().unwrap()),
            72,
            inputs,
        )
        .unwrap();
        baseline
            .step(guard(&baseline), GameCommand::default())
            .unwrap();
        authored
            .step(guard(&authored), GameCommand::default())
            .unwrap();
        let baseline_snapshot = baseline.snapshot().value;
        let authored_snapshot = authored.snapshot().value;
        for (left, right) in [
            (authored_snapshot.tick, baseline_snapshot.tick),
            (
                u64::from(authored_snapshot.objective_count),
                u64::from(baseline_snapshot.objective_count),
            ),
            (
                authored_snapshot.action_sequence,
                baseline_snapshot.action_sequence,
            ),
        ] {
            assert_eq!(left, right);
        }
        assert_eq!(authored_snapshot.player_pose, baseline_snapshot.player_pose);
        assert_eq!(
            authored_snapshot.player_motion,
            baseline_snapshot.player_motion
        );
        assert_eq!(authored_snapshot.caster_mana, baseline_snapshot.caster_mana);
        assert_eq!(
            authored.backend().projection().mission.global_rng(),
            baseline.backend().projection().mission.global_rng()
        );
        assert_eq!(
            authored.backend().projection().live_world.houses(),
            baseline.backend().projection().live_world.houses()
        );
        assert_eq!(
            authored.backend().projection().live_world.villagers(),
            baseline.backend().projection().live_world.villagers()
        );

        let baseline_events = baseline
            .events_after(guard(&baseline), 0, 1_024)
            .unwrap()
            .value
            .events;
        let authored_events = authored
            .events_after(guard(&authored), 0, 1_024)
            .unwrap()
            .value
            .events;
        assert_eq!(
            baseline_events
                .iter()
                .filter(|event| event.event_id.as_str() == "ai.engagement_boundary")
                .count(),
            0
        );
        let authored_engagements = authored_events
            .iter()
            .filter(|event| event.event_id.as_str() == "ai.engagement_boundary")
            .collect::<Vec<_>>();
        assert!(!authored_engagements.is_empty());
        assert!(
            authored_engagements
                .iter()
                .all(|event| event.actor.is_some() && event.participant.is_some())
        );

        let baseline_goats = baseline.backend().projection().live_world.goats();
        let authored_goats = authored.backend().projection().live_world.goats();
        let player_entity_index = authored.backend().projection().state.entity_index;
        let mut changed = 0;
        for (baseline_goat, authored_goat) in baseline_goats.iter().zip(authored_goats) {
            if baseline_goat == authored_goat {
                continue;
            }
            changed += 1;
            assert_eq!(authored_goat.action, 14);
            assert_eq!(authored_goat.stage_var_2, 10);
            assert_eq!(authored_goat.target_entity_index, Some(player_entity_index));
            assert_eq!(authored_goat.rng, baseline_goat.rng);
            let mut normalized = authored_goat.clone();
            normalized.action = baseline_goat.action;
            normalized.stage_var_2 = baseline_goat.stage_var_2;
            normalized.target_entity_index = baseline_goat.target_entity_index;
            normalized.actual_speed = baseline_goat.actual_speed;
            assert_eq!(&normalized, baseline_goat);
        }
        assert_eq!(changed, authored_engagements.len());

        authored.reset(guard(&authored)).unwrap();
        authored
            .step(guard(&authored), GameCommand::default())
            .unwrap();
        assert_eq!(authored.snapshot().value, authored_snapshot);

        store
            .rollback(
                &applied.resulting_definition_hash,
                applied.resulting_definition_revision,
                &applied.transaction_id,
                "restore-classic-goat-policy",
                PublicationFault::None,
            )
            .unwrap();
        authored
            .reopen(
                guard(&authored),
                Arc::new(store.active_definition().unwrap()),
            )
            .unwrap();
        baseline.reset(guard(&baseline)).unwrap();
        authored
            .step(guard(&authored), GameCommand::default())
            .unwrap();
        baseline
            .step(guard(&baseline), GameCommand::default())
            .unwrap();
        assert_eq!(authored.snapshot().value, baseline.snapshot().value);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sealed_level1_open_step_reset_replays_existing_hashes() {
        let mut session = open_fixture();
        let initial = session.snapshot();
        let command = PlayerCommand::default();

        let first = CampaignFacade::<SealedLevel1Adapter>::step(&mut session, command)
            .expect("sealed Level 1 facade step");
        let reset = CampaignFacade::<SealedLevel1Adapter>::reset(&mut session)
            .expect("sealed Level 1 facade reset");
        let replay = CampaignFacade::<SealedLevel1Adapter>::step(&mut session, command)
            .expect("sealed Level 1 facade replay");

        assert_eq!(reset.state_hash, initial.state_hash);
        assert_eq!(reset.conformance_hash, initial.conformance_hash);
        assert_eq!(reset.event_head, initial.event_head);
        assert_ne!(reset.generation, initial.generation);
        assert_eq!(replay.state_hash, first.state_hash);
        assert_eq!(replay.conformance_hash, first.conformance_hash);
        assert_eq!(replay.tick, first.tick);
        assert_eq!(replay.event_head, first.event_head);
    }

    #[test]
    fn invalid_key_fails_before_load_or_existing_session_mutation() {
        let mut control = open_fixture();
        let command = PlayerCommand {
            flags: 1,
            ..PlayerCommand::default()
        };
        let _ = CampaignFacade::<SealedLevel1Adapter>::step(&mut control, command)
            .expect("control session advances before the negative");
        let before = control.snapshot();
        let before_events = control.events.clone();

        let invalid = InternalCampaignKey {
            external_level: 2,
            ..SealedLevel1Adapter::KEY
        };
        let error = CampaignFacade::<SealedLevel1Adapter>::open(
            invalid,
            InternalCampaignOpen {
                package_dir: Path::new("/must-not-load/invalid-campaign-package"),
                closure_path: Some(Path::new("/must-not-load/invalid-campaign-closure")),
                owner_cache: None,
            },
        )
        .expect_err("an unknown internal campaign key must fail closed");

        assert_eq!(error.status, MC_STATUS_VALIDATION_FAILED);
        assert_eq!(error.message, "unknown internal campaign key 2/0/1");
        assert_eq!(control.snapshot(), before);
        assert_eq!(control.events, before_events);
    }
}
