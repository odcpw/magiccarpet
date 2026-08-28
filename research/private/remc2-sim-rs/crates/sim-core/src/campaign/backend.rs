//! Private MC2 campaign adapter behind the neutral game-session seam.
//!
//! This module makes descriptor-backed campaign execution a
//! [`GameSessionBackend`] so campaign play crosses the same neutral
//! [`crate::game_session::GameSession`] lifecycle as every other package. The
//! qualified [`CampaignSession`] remains the one live Rust authority; its
//! source-shaped carriers, slots, selectors, and schedule stay private to
//! this adapter. No public identifier names an external or internal level
//! number, and no source slot or class/model/action value crosses the
//! neutral contract.

use std::{
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};
use sim_definition::adapter::{
    artifact_node, definition_error, game_package, owner_provenance, with_level_definition,
};

use crate::{
    FlightEvent, PlayerCommand,
    campaign::{
        CampaignOpenError, CampaignSession,
        dispatch::CAMPAIGN_BEHAVIOR_REGISTRY,
        journal::{CampaignEffect, EffectCursor, EffectPageError, EffectRecord},
        spec::{CampaignDefinition as CampaignDescriptor, CampaignSelection},
    },
    game_definition::{
        ArtifactKind, DefinitionError, DefinitionErrorCode, DefinitionFlightBinding,
        GameDefinition, RequiredCapability, SemanticCatalog, SemanticId, default_catalog,
        engine_capabilities,
    },
    game_session::{
        CampaignSnapshot, EventPage, GameAdapter, GameAudioCommand, GameCommand, GameEvent,
        GameSessionBackend, LateralControl, LongitudinalControl, PlayerMotion, PlayerPose,
        RuntimeActorId, RuntimeParticipantId,
    },
    native_world::{Level2DispatchEvent, Level2SharedSpellEvent},
};

/// The qualified campaign route runs the source single-pass schedule. Speed
/// selection is not yet a discoverable descriptor, so the adapter pins the
/// qualified value instead of exposing an unvalidated source knob.
const CAMPAIGN_SPEED_INDEX: u8 = 0;

/// Typed failure surface for the campaign adapter and backend.
#[derive(Debug)]
pub enum CampaignBackendError {
    /// Campaign package/descriptor preflight or materialization failed.
    Open(CampaignOpenError),
    /// Neutral definition validation failed.
    Definition(DefinitionError),
    /// The definition carries a semantic value with no campaign consumption
    /// binding; accepting it would claim an unimplemented mutation.
    UnappliedSemanticValue { id: SemanticId },
    /// Session reset cannot substitute a package or artifact graph.
    PackageSubstitution,
    /// The requested event cursor is beyond the current effect head.
    CursorBeyondHead { head: u64, requested: u64 },
    /// The requested event cursor predates the retained journal.
    CursorExpired {
        first_available: u64,
        requested: u64,
    },
    /// The campaign adapter retains no positional audio sidecar.
    NoAudioSidecar { event_sequence: u64 },
    /// The source schedule rejected the step; the session is unchanged.
    StepRejected { message: String },
    /// Internal journal/cursor invariant failure.
    JournalDesync,
}

impl fmt::Display for CampaignBackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open(error) => write!(formatter, "campaign open failed: {error}"),
            Self::Definition(error) => error.fmt(formatter),
            Self::UnappliedSemanticValue { id } => write!(
                formatter,
                "UNAPPLIED_SEMANTIC_VALUE: {} has no campaign consumption binding; \
                 only catalog default values are accepted",
                id.as_str()
            ),
            Self::PackageSubstitution => formatter.write_str(
                "session reset cannot substitute a package or artifact graph; reopen resources first",
            ),
            Self::CursorBeyondHead { head, requested } => write!(
                formatter,
                "event cursor {requested} is beyond current event head {head}"
            ),
            Self::CursorExpired {
                first_available,
                requested,
            } => write!(
                formatter,
                "event cursor {requested} predates the retained journal; first available cursor is {first_available}"
            ),
            Self::NoAudioSidecar { event_sequence } => write!(
                formatter,
                "event sequence {event_sequence} has no positional audio sidecar on the campaign route"
            ),
            Self::StepRejected { message } => {
                write!(formatter, "campaign schedule rejected the step: {message}")
            }
            Self::JournalDesync => {
                formatter.write_str("campaign journal cursor invariant failure")
            }
        }
    }
}

impl Error for CampaignBackendError {}

/// Validate the campaign descriptor package and adapt it into one neutral
/// immutable definition. Descriptor selectors terminate here; the resulting
/// definition exposes only semantic IDs and content identities.
///
/// # Errors
///
/// Returns a typed error if descriptor preflight, package enumeration,
/// capability resolution, or definition materialization fails. No definition
/// is created on error.
pub fn load_mc2_campaign(
    package_root: impl AsRef<Path>,
    catalog: &SemanticCatalog,
) -> crate::game_definition::Result<GameDefinition> {
    let package_root = package_root.as_ref();
    let selection = CampaignDescriptor::declared_selection(package_root).map_err(|error| {
        definition_error(
            DefinitionErrorCode::InvalidSchema,
            "mc2_campaign.package",
            format!("cannot read campaign selection: {error}"),
        )
    })?;
    let descriptor =
        CampaignDescriptor::preflight(package_root, selection, &CAMPAIGN_BEHAVIOR_REGISTRY)
            .map_err(|error| {
                definition_error(
                    DefinitionErrorCode::InvalidSchema,
                    "mc2_campaign.package",
                    format!("campaign descriptor preflight failed: {error}"),
                )
            })?;
    let export_sha256 = descriptor.owner_export_package_sha256().map_err(|error| {
        definition_error(
            DefinitionErrorCode::HashMismatch,
            "mc2_campaign.owner_export",
            format!("cannot reconstruct wrapped owner export identity: {error}"),
        )
    })?;
    let (package_bytes, envelope_bytes) = campaign_package_bytes(package_root)?;
    let artifacts = vec![
        artifact_node(
            "campaign.owner_export",
            ArtifactKind::World,
            "campaign/owner-export",
            &export_sha256,
            package_bytes - envelope_bytes,
            &[],
        )?,
        artifact_node(
            "campaign.package",
            ArtifactKind::Definition,
            "campaign/package",
            &descriptor.package_sha256,
            package_bytes,
            &["campaign.owner_export"],
        )?,
    ];
    let requirements = [
        "engine.ai.creature_policy",
        "engine.flight.fixed_point",
        "engine.spell.projectile",
    ]
    .into_iter()
    .map(|id| {
        Ok(RequiredCapability {
            id: SemanticId::parse(id)?,
            exact_version: 1,
        })
    })
    .collect::<crate::game_definition::Result<Vec<_>>>()?;
    let package = game_package(
        "mc2.campaign",
        1,
        requirements,
        artifacts,
        Vec::new(),
        owner_provenance(&descriptor.package_sha256)?,
    )?;
    GameDefinition::from_package(package, &engine_capabilities()?, catalog)
        .map(|definition| with_level_definition(definition, descriptor.level_definition().clone()))
}

fn campaign_package_bytes(root: &Path) -> crate::game_definition::Result<(u64, u64)> {
    let mut total = 0_u64;
    let mut envelope = 0_u64;
    for entry in fs::read_dir(root).map_err(|error| {
        definition_error(
            DefinitionErrorCode::Io,
            "mc2_campaign.package",
            format!("cannot enumerate campaign package: {error}"),
        )
    })? {
        let entry = entry.map_err(|error| {
            definition_error(
                DefinitionErrorCode::Io,
                "mc2_campaign.package",
                format!("cannot inspect campaign package: {error}"),
            )
        })?;
        let metadata = entry.metadata().map_err(|error| {
            definition_error(
                DefinitionErrorCode::Io,
                "mc2_campaign.package",
                format!("cannot inspect campaign package member: {error}"),
            )
        })?;
        let file_type = entry.file_type().map_err(|error| {
            definition_error(
                DefinitionErrorCode::Io,
                "mc2_campaign.package",
                error.to_string(),
            )
        })?;
        if !metadata.is_file() || file_type.is_symlink() {
            return Err(definition_error(
                DefinitionErrorCode::PathEscape,
                entry.path().display().to_string(),
                "campaign package contains a non-regular or symlink member",
            ));
        }
        total = total.checked_add(metadata.len()).ok_or_else(|| {
            definition_error(
                DefinitionErrorCode::OutOfRange,
                "mc2_campaign.package",
                "campaign package byte total overflows u64",
            )
        })?;
        let name = entry.file_name();
        if name == "manifest.json" || name == "campaign-definition.json" {
            envelope += metadata.len();
        }
    }
    Ok((total, envelope))
}

/// Adapter that opens the private MC2 campaign route behind the neutral
/// session contract.
pub struct Mc2CampaignAdapter;

/// Owner-local campaign package inputs. Owner bytes stay outside the neutral
/// definition and the distributable client tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mc2CampaignInputs {
    pub package_root: PathBuf,
    pub owner_cache: Option<PathBuf>,
}

impl Mc2CampaignInputs {
    #[must_use]
    pub fn new(package_root: impl Into<PathBuf>) -> Self {
        Self {
            package_root: package_root.into(),
            owner_cache: None,
        }
    }

    #[must_use]
    pub fn with_owner_cache(
        package_root: impl Into<PathBuf>,
        owner_cache: impl Into<PathBuf>,
    ) -> Self {
        Self {
            package_root: package_root.into(),
            owner_cache: Some(owner_cache.into()),
        }
    }
}

/// Definition-bound campaign backend. The wrapped [`CampaignSession`] owns
/// live gameplay state, ordering, RNG, journals, reset, and replay.
#[derive(Debug)]
pub struct Mc2CampaignBackend {
    campaign: CampaignSession,
    package_campaign: CampaignSession,
    flight_binding: DefinitionFlightBinding,
    package_base: GameDefinition,
    package_graph_hash: String,
    artifact_graph_hash: String,
}

impl GameAdapter for Mc2CampaignAdapter {
    type Backend = Mc2CampaignBackend;
    type OpenInput = Mc2CampaignInputs;
    type Error = CampaignBackendError;

    fn open_backend(
        &self,
        definition: &GameDefinition,
        input: Self::OpenInput,
    ) -> Result<Self::Backend, Self::Error> {
        let catalog = default_catalog().map_err(CampaignBackendError::Definition)?;
        let flight_binding = DefinitionFlightBinding::from_definition(definition)
            .map_err(CampaignBackendError::Definition)?;
        require_catalog_defaults(definition, &catalog)?;
        let package_base = load_mc2_campaign(&input.package_root, &catalog)
            .map_err(CampaignBackendError::Definition)?;
        definition
            .verify_semantic_variant_of(&package_base, &catalog)
            .map_err(CampaignBackendError::Definition)?;
        let selection = CampaignDescriptor::declared_selection(&input.package_root)
            .map_err(|error| CampaignBackendError::Open(CampaignOpenError::Preflight(error)))?;
        let mut package_campaign = match &input.owner_cache {
            Some(owner_cache) => {
                CampaignSession::open_with_owner_cache(&input.package_root, owner_cache, selection)
            }
            None => CampaignSession::open(&input.package_root, selection),
        }
        .map_err(CampaignBackendError::Open)?;
        // The neutral session is the campaign route: bind the authored
        // campaign entry stage before the first generation is exposed, like
        // the retained `mc_native_campaign_*` opens. The pristine
        // generation-0 state rebases, so reset restores the entry-staged
        // baseline instead of the raw package selector.
        if selection == CampaignSelection::LEVEL2 {
            package_campaign
                .level2_compatibility_mut()
                .select_level2_campaign_entry_stage();
        }
        let mut campaign = package_campaign.clone();
        campaign
            .apply_semantic_level_variant(
                definition
                    .level_definition()
                    .expect("campaign semantic variant retains its level definition"),
            )
            .map_err(|error| {
                CampaignBackendError::Open(CampaignOpenError::Materialization(error))
            })?;
        Ok(Mc2CampaignBackend {
            campaign,
            package_campaign,
            flight_binding,
            package_graph_hash: definition.package_graph_hash().to_owned(),
            artifact_graph_hash: definition.artifact_graph_hash().to_owned(),
            package_base,
        })
    }
}

/// No campaign descriptor has a definition-consumption binding yet, so a
/// deviated semantic value cannot reach gameplay. Fail closed instead of
/// silently binding a definition whose values this route would ignore.
fn require_catalog_defaults(
    definition: &GameDefinition,
    catalog: &SemanticCatalog,
) -> Result<(), CampaignBackendError> {
    for descriptor in catalog.descriptors() {
        if DefinitionFlightBinding::consumes(&descriptor.id) {
            continue;
        }
        if definition.value(&descriptor.id) != Some(&descriptor.default_value) {
            return Err(CampaignBackendError::UnappliedSemanticValue {
                id: descriptor.id.clone(),
            });
        }
    }
    Ok(())
}

impl GameSessionBackend for Mc2CampaignBackend {
    type Command = GameCommand;
    type Snapshot = CampaignSnapshot;
    type Error = CampaignBackendError;

    fn tick(&self) -> u64 {
        self.campaign.tick()
    }

    fn snapshot(&self) -> Self::Snapshot {
        let flight = &self.campaign.level2.state;
        CampaignSnapshot {
            tick: flight.tick,
            effect_head: self.campaign.effect_cursor().next_sequence,
            state_hash: self.campaign.state_hash(),
            terrain_hash: self.campaign.terrain_hash(),
            effect_journal_sha256: hex_digest(
                self.campaign.level2.effect_journal().receipt().as_bytes(),
            ),
            player_pose: PlayerPose {
                x: flight.position.x,
                y: flight.position.y,
                z: flight.position.z,
                yaw: flight.yaw,
                display_pitch: flight.display_pitch,
                effective_pitch: flight.effective_pitch,
                roll: flight.roll_accumulator,
            },
            player_motion: PlayerMotion {
                target_speed: flight.target_speed,
                actual_speed: flight.actual_speed,
                strafe_speed: flight.strafe_speed,
            },
        }
    }

    fn events_after(&self, cursor: u64, limit: usize) -> Result<EventPage, Self::Error> {
        let head = self.campaign.effect_cursor().next_sequence;
        let requested = EffectCursor {
            generation: self.campaign.generation(),
            next_sequence: cursor,
        };
        let page = match self.campaign.effects_after(requested, limit.max(1)) {
            Ok(page) => page,
            Err(EffectPageError::FutureCursor { requested, .. }) => {
                return Err(CampaignBackendError::CursorBeyondHead { head, requested });
            }
            Err(EffectPageError::ExpiredCursor {
                first_available,
                requested,
            }) => {
                return Err(CampaignBackendError::CursorExpired {
                    first_available,
                    requested,
                });
            }
            Err(
                EffectPageError::StaleGeneration { .. } | EffectPageError::OutputTooSmall { .. },
            ) => {
                return Err(CampaignBackendError::JournalDesync);
            }
        };
        let records: &[EffectRecord] = if limit == 0 { &[] } else { &page.records };
        let events = records
            .iter()
            .map(|record| self.game_event(record))
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = events.last().map_or(cursor, |event| event.sequence);
        Ok(EventPage {
            requested_cursor: cursor,
            next_cursor,
            event_head: head,
            has_more: next_cursor < head,
            events,
            compatibility: None,
        })
    }

    fn audio_for_event(&self, event_sequence: u64) -> Result<GameAudioCommand, Self::Error> {
        Err(CampaignBackendError::NoAudioSidecar { event_sequence })
    }

    fn step(&mut self, command: Self::Command) -> Result<(), Self::Error> {
        let mut candidate = self.campaign.clone();
        let _evidence = candidate.step_with_tuning(
            campaign_player_command(command),
            CAMPAIGN_SPEED_INDEX,
            self.flight_binding.tuning(),
        );
        if let Some(message) = candidate.events().iter().find_map(|event| match event {
            Level2DispatchEvent::Error { message, .. } => Some(message.clone()),
            _ => None,
        }) {
            return Err(CampaignBackendError::StepRejected { message });
        }
        self.campaign = candidate;
        Ok(())
    }

    fn reset_for_definition(&mut self, definition: &GameDefinition) -> Result<(), Self::Error> {
        if definition.package_graph_hash() != self.package_graph_hash
            || definition.artifact_graph_hash() != self.artifact_graph_hash
        {
            return Err(CampaignBackendError::PackageSubstitution);
        }
        let catalog = default_catalog().map_err(CampaignBackendError::Definition)?;
        let flight_binding = DefinitionFlightBinding::from_definition(definition)
            .map_err(CampaignBackendError::Definition)?;
        definition
            .verify_semantic_variant_of(&self.package_base, &catalog)
            .map_err(CampaignBackendError::Definition)?;
        require_catalog_defaults(definition, &catalog)?;
        let next_generation = self.campaign.generation().wrapping_add(1);
        let mut campaign = self.package_campaign.clone();
        campaign
            .apply_semantic_level_variant(
                definition
                    .level_definition()
                    .expect("campaign semantic variant retains its level definition"),
            )
            .map_err(|error| {
                CampaignBackendError::Open(CampaignOpenError::Materialization(error))
            })?;
        campaign.set_runtime_generation(next_generation);
        self.campaign = campaign;
        self.flight_binding = flight_binding;
        Ok(())
    }
}

impl Mc2CampaignBackend {
    /// Read-only view of this backend's one live campaign authority for the
    /// retained `mc_native_campaign_*` presentation projections. No mutable
    /// counterpart exists: the neutral session lifecycle stays the only
    /// campaign gameplay writer.
    #[doc(hidden)]
    #[must_use]
    pub fn compatibility_campaign(&self) -> &CampaignSession {
        &self.campaign
    }

    fn opaque_runtime_handle(&self, namespace: &[u8], source_value: u64) -> u64 {
        let mut digest = Sha256::new();
        digest.update(b"mc2-campaign-runtime-handle/v1\0");
        digest.update(self.campaign.definition().package_sha256.as_bytes());
        digest.update([0]);
        digest.update(namespace);
        digest.update([0]);
        digest.update(source_value.to_le_bytes());
        let bytes: [u8; 8] = digest.finalize()[..8]
            .try_into()
            .expect("SHA-256 prefix is eight bytes");
        u64::from_le_bytes(bytes).max(1)
    }

    fn actor(&self, source_value: u64) -> Option<RuntimeActorId> {
        Some(RuntimeActorId(
            self.opaque_runtime_handle(b"actor", source_value),
        ))
    }

    fn participant(&self, source_value: u64) -> Option<RuntimeParticipantId> {
        Some(RuntimeParticipantId(
            self.opaque_runtime_handle(b"participant", source_value),
        ))
    }

    fn game_event(&self, record: &EffectRecord) -> Result<GameEvent, CampaignBackendError> {
        let (event_id, actor, participant) = match &record.effect {
            CampaignEffect::Dispatch(event) => match event {
                Level2DispatchEvent::Error { .. } => ("session.dispatch_fault", None, None),
                _ => {
                    let (_, _, _, _, source_slot) = event.campaign_projection();
                    (
                        "actor.behavior_dispatched",
                        self.actor(source_slot.into()),
                        None,
                    )
                }
            },
            CampaignEffect::SharedSpell(event) => match *event {
                Level2SharedSpellEvent::SpellCollected {
                    source_slot,
                    player_entity_index,
                    ..
                } => (
                    "spell.collected",
                    self.actor(source_slot.into()),
                    self.participant(player_entity_index.into()),
                ),
                Level2SharedSpellEvent::SpellAcquired {
                    source_slot,
                    player_entity_index,
                    ..
                } => (
                    "spell.acquired",
                    self.actor(source_slot.into()),
                    self.participant(player_entity_index.into()),
                ),
                Level2SharedSpellEvent::ShieldDamageApplied {
                    target_entity_index,
                    ..
                } => (
                    "combat.actor_damaged",
                    self.actor(target_entity_index.into()),
                    None,
                ),
                Level2SharedSpellEvent::ExperienceScrollCollected {
                    source_slot,
                    player_entity_index,
                } => (
                    "progression.experience_collected",
                    self.actor(source_slot.into()),
                    self.participant(player_entity_index.into()),
                ),
                Level2SharedSpellEvent::AbilityImproved { .. } => ("ability.improved", None, None),
            },
            CampaignEffect::Flight(event) => match *event {
                FlightEvent::PrepareSound {
                    entity_index,
                    player_index,
                    ..
                } => (
                    "audio.sound_prepared",
                    (entity_index != 0)
                        .then(|| self.actor(entity_index.into()))
                        .flatten(),
                    (player_index >= 0)
                        .then(|| self.participant(player_index.cast_unsigned().into()))
                        .flatten(),
                ),
                FlightEvent::EndLoop {
                    entity_index,
                    player_index,
                    ..
                } => (
                    "audio.loop_ended",
                    (entity_index != 0)
                        .then(|| self.actor(entity_index.into()))
                        .flatten(),
                    (player_index >= 0)
                        .then(|| self.participant(player_index.cast_unsigned().into()))
                        .flatten(),
                ),
                FlightEvent::UpdateMusic { .. } => ("audio.music_updated", None, None),
                FlightEvent::RequestSpellActivation {
                    holder_entity_index,
                    ..
                } => (
                    "spell.activation_requested",
                    self.actor(holder_entity_index.into()),
                    None,
                ),
                FlightEvent::ShowHelpHint { .. } => ("ui.help_hint_requested", None, None),
            },
            CampaignEffect::SceneChange { .. } => ("session.scene_changed", None, None),
        };
        Ok(GameEvent {
            sequence: record.sequence.wrapping_add(1),
            // The journal retains exact effect content, not per-effect state
            // fingerprints; the authoritative state hash lives on the
            // snapshot, so this field stays at its unbound zero value.
            state_hash: 0,
            event_id: SemanticId::parse(event_id).map_err(CampaignBackendError::Definition)?,
            actor,
            participant,
            asset: None,
        })
    }
}

fn campaign_player_command(command: GameCommand) -> PlayerCommand {
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
        roll: command.roll_axis,
        pitch: command.pitch_axis,
        flags: flags.cast_signed(),
        argument_6: command.look_yaw.cast_unsigned(),
        argument_8: command.look_pitch.cast_unsigned(),
        ..PlayerCommand::default()
    }
}

fn hex_digest(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign::state::{ResetState, encode_reset_v1};
    use crate::game_definition::{
        ChangeSetBuilder, GamePackage, IntegerParameter, PublicationFault, RequestedTiming,
        SemanticValue, WorkspaceStore,
    };
    use crate::game_session::{GameSession, SessionGuard};
    use crate::level_definition::{
        ActivationGroupId, BuildingKind, CreatureKind, ItemKind, LevelEntityKind,
        LevelEntityParameterAccess, LevelObjectiveConditionAccess, MechanicKind,
        ObjectiveCondition, SceneryKind, SpellKind, SwitchKind, TilePosition,
    };
    use sim_definition::adapter::{
        build_level_definition, entity_definition, objective_definition, reissue_at_revision,
        with_replacement,
    };
    use std::{
        collections::BTreeSet,
        path::PathBuf,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn descriptor_fixture_path() -> PathBuf {
        std::env::var_os("MC_CAMPAIGN_PACKAGE_V1").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf02-campaign-package-v1-a/qualified-level2")
            },
            PathBuf::from,
        )
    }

    fn level_three_descriptor_fixture_path() -> PathBuf {
        std::env::var_os("MC_CAMPAIGN_PACKAGE_LEVEL3_V1").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf14-campaign-package-v1-a/qualified-level3")
            },
            PathBuf::from,
        )
    }

    fn level_one_descriptor_fixture_path() -> PathBuf {
        std::env::var_os("MC_CAMPAIGN_LEVEL1_PACKAGE_V1").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf15-level1-package-current")
            },
            PathBuf::from,
        )
    }

    fn temporary_workspace_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock follows the Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "carpet-flight-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn publish_player_max_forward_speed(
        store: &WorkspaceStore,
        catalog: &SemanticCatalog,
        active: &GameDefinition,
        idempotency_key: &str,
        speed: i64,
    ) -> GameDefinition {
        let change_set = ChangeSetBuilder::new(
            "campaign-flight-authoring",
            active,
            idempotency_key,
            RequestedTiming::SessionReset,
        )
        .set_integer(IntegerParameter::PlayerMaxForwardSpeed, speed)
        .build()
        .expect("typed flight change set");
        let validation = store
            .validate(catalog, change_set.clone())
            .expect("flight change validates");
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .expect("flight change publishes");
        store.active_definition().expect("published definition")
    }

    fn publish_start_trigger_move(
        store: &WorkspaceStore,
        catalog: &SemanticCatalog,
        active: &GameDefinition,
        idempotency_key: &str,
        position: TilePosition,
    ) -> GameDefinition {
        let entity = sim_definition::adapter::stable_entity_id("switch.start_area_exit").unwrap();
        let change_set = ChangeSetBuilder::new(
            "campaign-placement-authoring",
            active,
            idempotency_key,
            RequestedTiming::SessionReset,
        )
        .move_entity_from(entity, TilePosition { x: 74, y: 212 }, position)
        .build()
        .expect("typed placement change set");
        let validation = store
            .validate(catalog, change_set.clone())
            .expect("placement change validates");
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .expect("placement change publishes");
        store.active_definition().expect("published definition")
    }

    fn publish_reach_objective(
        store: &WorkspaceStore,
        catalog: &SemanticCatalog,
        active: &GameDefinition,
        workspace: &str,
        objective_id: &str,
        original: ObjectiveCondition,
        authored: ObjectiveCondition,
    ) -> GameDefinition {
        let objective = sim_definition::adapter::stable_objective_id(objective_id).unwrap();
        let change_set = ChangeSetBuilder::new(
            workspace,
            active,
            "author-reach-objective",
            RequestedTiming::SessionReset,
        )
        .put_objective_from(objective, original, authored)
        .build()
        .expect("typed objective change set");
        let validation = store
            .validate(catalog, change_set.clone())
            .expect("objective change validates");
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .expect("objective change publishes");
        store.active_definition().expect("published definition")
    }

    fn publish_countdown_delay(
        store: &WorkspaceStore,
        catalog: &SemanticCatalog,
        active: &GameDefinition,
        workspace: &str,
        entity_id: &str,
        original: u32,
        authored: u32,
    ) -> GameDefinition {
        let entity = sim_definition::adapter::stable_entity_id(entity_id).unwrap();
        let change_set = ChangeSetBuilder::new(
            workspace,
            active,
            "author-countdown-delay",
            RequestedTiming::SessionReset,
        )
        .set_mechanic_countdown_from(entity, original, authored)
        .build()
        .expect("typed countdown change set");
        let validation = store
            .validate(catalog, change_set.clone())
            .expect("countdown change validates");
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .expect("countdown change publishes");
        store.active_definition().expect("published definition")
    }

    fn assert_package_content(
        root: &Path,
        expected_creatures: &[(CreatureKind, usize)],
        expected_spells: &[(SpellKind, TilePosition, Option<u16>)],
        expected_scenery: &[(SceneryKind, usize)],
        expected_items: &[(ItemKind, usize)],
        expected_buildings: &[(BuildingKind, usize)],
        expected_switches: &[(SwitchKind, usize)],
        expected_mechanics: &[(MechanicKind, TilePosition, Option<u16>)],
        expected_objectives: &[ObjectiveCondition],
        expected_activation_groups: &[u16],
    ) -> GameDefinition {
        let catalog = default_catalog().unwrap();
        let definition = load_mc2_campaign(root, &catalog).unwrap();
        let level = definition
            .level_definition()
            .expect("the campaign package exposes semantic level content");
        assert_eq!(
            level.creature_kinds().collect::<Vec<_>>(),
            expected_creatures
                .iter()
                .map(|(kind, _)| *kind)
                .collect::<Vec<_>>()
        );
        for (kind, count) in expected_creatures {
            assert_eq!(level.authored_creature_count(*kind), *count);
        }
        assert_eq!(
            level.spell_kinds().collect::<Vec<_>>(),
            expected_spells
                .iter()
                .map(|(kind, _, _)| *kind)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        );
        for kind in level.spell_kinds() {
            assert_eq!(
                level.authored_spell_pickup_count(kind),
                expected_spells
                    .iter()
                    .filter(|(expected_kind, _, _)| *expected_kind == kind)
                    .count()
            );
        }
        assert_eq!(
            level
                .entities()
                .iter()
                .filter_map(|entity| match entity.kind() {
                    LevelEntityKind::Creature(_)
                    | LevelEntityKind::Scenery(_)
                    | LevelEntityKind::Item(_)
                    | LevelEntityKind::Building(_)
                    | LevelEntityKind::Switch(_)
                    | LevelEntityKind::Mechanic(_) => None,
                    LevelEntityKind::SpellPickup(kind) => Some((
                        kind,
                        entity.position(),
                        entity.activation_group().map(ActivationGroupId::get),
                    )),
                })
                .collect::<Vec<_>>(),
            expected_spells
        );
        assert_eq!(
            level
                .scenery_kinds()
                .map(|kind| (kind, level.authored_scenery_count(kind)))
                .collect::<Vec<_>>(),
            expected_scenery
        );
        assert_eq!(
            level
                .item_kinds()
                .map(|kind| (kind, level.authored_item_count(kind)))
                .collect::<Vec<_>>(),
            expected_items
        );
        assert_eq!(
            level
                .building_kinds()
                .map(|kind| (kind, level.authored_building_count(kind)))
                .collect::<Vec<_>>(),
            expected_buildings
        );
        assert_eq!(
            level
                .switch_kinds()
                .map(|kind| (kind, level.authored_switch_count(kind)))
                .collect::<Vec<_>>(),
            expected_switches
        );
        assert_eq!(
            level.mechanic_kinds().collect::<Vec<_>>(),
            expected_mechanics
                .iter()
                .map(|(kind, _, _)| *kind)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        );
        for kind in level.mechanic_kinds() {
            assert_eq!(
                level.authored_mechanic_count(kind),
                expected_mechanics
                    .iter()
                    .filter(|(expected_kind, _, _)| *expected_kind == kind)
                    .count()
            );
        }
        assert_eq!(
            level
                .entities()
                .iter()
                .filter_map(|entity| match entity.kind() {
                    LevelEntityKind::Creature(_)
                    | LevelEntityKind::SpellPickup(_)
                    | LevelEntityKind::Scenery(_)
                    | LevelEntityKind::Item(_)
                    | LevelEntityKind::Building(_)
                    | LevelEntityKind::Switch(_) => None,
                    LevelEntityKind::Mechanic(kind) => Some((
                        kind,
                        entity.position(),
                        entity.activation_group().map(ActivationGroupId::get),
                    )),
                })
                .collect::<Vec<_>>(),
            expected_mechanics
        );
        assert_eq!(
            level.objective_conditions().collect::<Vec<_>>(),
            expected_objectives
        );
        assert_eq!(
            level
                .activation_groups()
                .map(ActivationGroupId::get)
                .collect::<Vec<_>>(),
            expected_activation_groups
        );
        definition
    }

    #[test]
    fn semantic_content_definitions_follow_level_one_two_and_three_packages() {
        let root = level_one_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let definition = assert_package_content(
            &root,
            &[
                (CreatureKind::Goat, 33),
                (CreatureKind::Worm, 4),
                (CreatureKind::Archer, 4),
                (CreatureKind::Villager, 28),
                (CreatureKind::Firefly, 6),
            ],
            &[
                (SpellKind::Castle, TilePosition { x: 194, y: 213 }, Some(1)),
                (SpellKind::Speed, TilePosition { x: 202, y: 212 }, Some(11)),
            ],
            &[
                (SceneryKind::Tree, 18),
                (SceneryKind::Stone, 1),
                (SceneryKind::ExitPortal, 1),
            ],
            &[(ItemKind::ExperienceScroll, 5)],
            &[(BuildingKind::House, 81)],
            &[
                (SwitchKind::AreaEntryTrigger, 24),
                (SwitchKind::AreaExitTrigger, 2),
                (SwitchKind::LevelExitGate, 1),
                (SwitchKind::ExitSequenceTrigger, 1),
                (SwitchKind::ObjectiveGate, 4),
            ],
            &[
                (
                    MechanicKind::AbsenceCountdown {
                        creature: CreatureKind::Archer,
                    },
                    TilePosition { x: 144, y: 255 },
                    Some(3),
                ),
                (
                    MechanicKind::AbsenceCountdown {
                        creature: CreatureKind::Worm,
                    },
                    TilePosition { x: 134, y: 98 },
                    Some(4),
                ),
            ],
            &[
                ObjectiveCondition::ReachLocation(TilePosition { x: 115, y: 212 }),
                ObjectiveCondition::ReachLocation(TilePosition { x: 194, y: 213 }),
                ObjectiveCondition::NoLivingCreatures(CreatureKind::Archer),
                ObjectiveCondition::CastleManaPercent(15),
                ObjectiveCondition::NoLivingCreatures(CreatureKind::Firefly),
            ],
            &[
                1, 2, 3, 4, 5, 6, 8, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
                26, 27, 28, 29, 30, 31, 32, 33, 34,
            ],
        );
        let level_one_countdowns = definition
            .level_definition()
            .unwrap()
            .entities()
            .iter()
            .filter_map(|entity| {
                entity.absence_countdown().map(|config| {
                    (
                        entity.id().as_str(),
                        config.default_arming_ticks(),
                        config.arming_ticks(),
                        config.arming_ticks_access(),
                    )
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            level_one_countdowns,
            vec![
                (
                    "mechanic.absence_countdown.archer.at_144_255",
                    16,
                    16,
                    LevelEntityParameterAccess::SessionReset,
                ),
                (
                    "mechanic.absence_countdown.worm.at_134_98",
                    16,
                    16,
                    LevelEntityParameterAccess::SessionReset,
                ),
            ]
        );
        let level_two = assert_package_content(
            &descriptor_fixture_path(),
            &[
                (CreatureKind::Goat, 36),
                (CreatureKind::Bee, 12),
                (CreatureKind::Worm, 6),
                (CreatureKind::Archer, 4),
                (CreatureKind::Skeleton, 6),
                (CreatureKind::Villager, 29),
                (CreatureKind::Trader, 27),
            ],
            &[
                (SpellKind::Shield, TilePosition { x: 138, y: 219 }, Some(22)),
                (
                    SpellKind::Invisibility,
                    TilePosition { x: 138, y: 237 },
                    Some(27),
                ),
                (SpellKind::BeyondSight, TilePosition { x: 1, y: 152 }, None),
                (SpellKind::Speed, TilePosition { x: 167, y: 59 }, None),
            ],
            &[
                (SceneryKind::Tree, 12),
                (SceneryKind::Stone, 1),
                (SceneryKind::ExitPortal, 1),
            ],
            &[(ItemKind::ExperienceScroll, 24)],
            &[(BuildingKind::House, 25)],
            &[
                (SwitchKind::AreaEntryTrigger, 17),
                (SwitchKind::AreaExitTrigger, 4),
                (SwitchKind::LevelExitGate, 1),
                (SwitchKind::ExitSequenceTrigger, 1),
                (SwitchKind::ObjectiveGate, 6),
            ],
            &[],
            &[
                ObjectiveCondition::ReachLocation(TilePosition { x: 2, y: 152 }),
                ObjectiveCondition::DefeatRivalWizard,
                ObjectiveCondition::NoLivingCreatures(CreatureKind::Bee),
                ObjectiveCondition::ReachLocation(TilePosition { x: 154, y: 229 }),
                ObjectiveCondition::NoStandingBuildings,
            ],
            &[
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
                24, 25, 26, 27, 28, 29, 31,
            ],
        );
        assert!(
            level_two
                .level_definition()
                .unwrap()
                .entities()
                .iter()
                .all(|entity| entity.absence_countdown().is_none())
        );
        let level_three = assert_package_content(
            &level_three_descriptor_fixture_path(),
            &[
                (CreatureKind::Dragon, 5),
                (CreatureKind::Goat, 11),
                (CreatureKind::Bee, 6),
                (CreatureKind::Worm, 5),
                (CreatureKind::Archer, 18),
                (CreatureKind::Skeleton, 11),
            ],
            &[
                (SpellKind::Shield, TilePosition { x: 85, y: 85 }, None),
                (
                    SpellKind::Invisibility,
                    TilePosition { x: 136, y: 85 },
                    None,
                ),
                (SpellKind::Morph, TilePosition { x: 110, y: 109 }, Some(4)),
            ],
            &[
                (SceneryKind::Tree, 37),
                (SceneryKind::Stone, 12),
                (SceneryKind::ExitPortal, 1),
            ],
            &[(ItemKind::ExperienceScroll, 10)],
            &[(BuildingKind::House, 97)],
            &[
                (SwitchKind::AreaEntryTrigger, 4),
                (SwitchKind::LevelExitGate, 1),
                (SwitchKind::ExitSequenceTrigger, 1),
                (SwitchKind::ObjectiveGate, 5),
            ],
            &[(
                MechanicKind::AbsenceCountdown {
                    creature: CreatureKind::Worm,
                },
                TilePosition { x: 160, y: 106 },
                None,
            )],
            &[
                ObjectiveCondition::NoLivingCreatures(CreatureKind::Skeleton),
                ObjectiveCondition::NoLivingCreatures(CreatureKind::Dragon),
                ObjectiveCondition::CastleManaPercent(30),
                ObjectiveCondition::ReachLocation(TilePosition { x: 110, y: 110 }),
                ObjectiveCondition::NoLivingCreatures(CreatureKind::Bee),
            ],
            &[1, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12],
        );
        assert_eq!(
            level_three
                .level_definition()
                .unwrap()
                .entities()
                .iter()
                .filter_map(|entity| entity.absence_countdown().map(|config| (
                    entity.id().as_str(),
                    config.default_arming_ticks(),
                    config.arming_ticks(),
                    config.arming_ticks_access(),
                )))
                .collect::<Vec<_>>(),
            vec![(
                "mechanic.absence_countdown.worm.at_160_106",
                16,
                16,
                LevelEntityParameterAccess::SessionReset,
            )]
        );
        let level = definition
            .level_definition()
            .expect("the campaign package exposes semantic level content");

        let swapped = level
            .entities()
            .iter()
            .map(|entity| match entity.kind() {
                LevelEntityKind::Creature(kind) => {
                    let wrong_kind = match kind {
                        CreatureKind::Archer => CreatureKind::Firefly,
                        _ => CreatureKind::Archer,
                    };
                    entity_definition(
                        entity.id().clone(),
                        LevelEntityKind::Creature(wrong_kind),
                        entity.position(),
                        entity.activation_group(),
                    )
                }
                LevelEntityKind::SpellPickup(_)
                | LevelEntityKind::Scenery(_)
                | LevelEntityKind::Item(_)
                | LevelEntityKind::Building(_)
                | LevelEntityKind::Switch(_)
                | LevelEntityKind::Mechanic(_) => entity.clone(),
            })
            .collect();
        let activation_groups = level.activation_groups().collect::<Vec<_>>();
        let tampered = with_level_definition(
            definition.clone(),
            build_level_definition(
                swapped,
                level.objectives().to_vec(),
                activation_groups.clone(),
            )
            .unwrap(),
        );
        let error = Mc2CampaignAdapter
            .open_backend(&tampered, Mc2CampaignInputs::new(&root))
            .expect_err("raw and semantic creature identities must not diverge");
        assert!(matches!(error, CampaignBackendError::Definition(_)));

        let mut wrong_objectives = level.objectives().to_vec();
        let wrong_objective_id = wrong_objectives[0].id().clone();
        wrong_objectives[0] = objective_definition(
            wrong_objective_id,
            ObjectiveCondition::CastleManaPercent(99),
            LevelObjectiveConditionAccess::ReadOnly,
        );
        let tampered = with_level_definition(
            definition.clone(),
            build_level_definition(
                level.entities().to_vec(),
                wrong_objectives,
                activation_groups,
            )
            .unwrap(),
        );
        let error = Mc2CampaignAdapter
            .open_backend(&tampered, Mc2CampaignInputs::new(&root))
            .expect_err("raw and semantic objective conditions must not diverge");
        assert!(matches!(error, CampaignBackendError::Definition(_)));

        let alias_package = game_package(
            "mc2.campaign.alias",
            definition.package_version(),
            definition.required_capabilities().to_vec(),
            definition.artifacts().to_vec(),
            Vec::new(),
            definition.provenance().clone(),
        )
        .unwrap();
        let alias_definition = with_level_definition(
            GameDefinition::from_package(alias_package, &engine_capabilities().unwrap(), &catalog)
                .unwrap(),
            level.clone(),
        );
        assert_eq!(alias_definition.level_definition(), Some(level));
        let error = Mc2CampaignAdapter
            .open_backend(&alias_definition, Mc2CampaignInputs::new(&root))
            .expect_err("matching semantic content must not bypass package identity");
        assert!(matches!(error, CampaignBackendError::Definition(_)));

        Mc2CampaignAdapter
            .open_backend(&definition, Mc2CampaignInputs::new(&root))
            .expect("the package-derived semantic definition opens the shared campaign engine");
    }

    #[test]
    fn semantic_objective_ids_and_access_follow_first_three_packages() {
        let catalog = default_catalog().unwrap();
        let cases = [
            (
                level_one_descriptor_fixture_path(),
                [
                    "objective.reach.my_beacon",
                    "objective.reach.foot_of_spire",
                    "objective.clear.spire_archers",
                    "objective.charge.first_castle",
                    "objective.clear.lethal_fireflies",
                ],
                [true, true, false, false, false],
            ),
            (
                descriptor_fixture_path(),
                [
                    "objective.reach.sight_beacon",
                    "objective.defeat.nyphur",
                    "objective.clear.plague_bees",
                    "objective.reach.payahandra_ruins",
                    "objective.clear.payahandra_vaults",
                ],
                [true, false, false, true, false],
            ),
            (
                level_three_descriptor_fixture_path(),
                [
                    "objective.clear.undead_horde",
                    "objective.clear.guardian_dragons",
                    "objective.charge.reveal_entrance",
                    "objective.reach.revealed_way",
                    "objective.clear.final_bees",
                ],
                [false, false, false, true, false],
            ),
        ];
        let mut writable = 0;
        for (root, expected_ids, expected_writable) in cases {
            let definition = load_mc2_campaign(root, &catalog).unwrap();
            let objectives = definition.level_definition().unwrap().objectives();
            assert_eq!(objectives.len(), 5);
            assert_eq!(
                objectives
                    .iter()
                    .map(|objective| objective.id().as_str())
                    .collect::<Vec<_>>(),
                expected_ids
            );
            for (objective, expected_writable) in
                objectives.iter().zip(expected_writable.into_iter())
            {
                assert_eq!(
                    objective.condition_access() == LevelObjectiveConditionAccess::SessionReset,
                    expected_writable
                );
                writable += usize::from(expected_writable);
            }
        }
        assert_eq!(writable, 5);
    }

    fn guard(session: &GameSession<Mc2CampaignBackend>) -> SessionGuard {
        let identity = session.identity();
        SessionGuard {
            session_id: identity.session_id,
            generation: identity.generation,
            expected_tick: identity.tick,
        }
    }

    fn assert_matches_direct(neutral: &GameSession<Mc2CampaignBackend>, direct: &CampaignSession) {
        let backend = neutral.backend();
        assert_eq!(backend.campaign.tick(), direct.tick());
        assert_eq!(neutral.identity().tick, direct.tick());
        assert_eq!(backend.campaign.state_hash(), direct.state_hash());
        assert_eq!(backend.campaign.terrain_hash(), direct.terrain_hash());
        assert_eq!(
            backend.campaign.level2.economy().reset_hash(),
            direct.level2.economy().reset_hash()
        );
        assert_eq!(
            backend.campaign.level2.source_global_rng(),
            direct.level2.source_global_rng()
        );
        assert_eq!(
            backend.campaign.level2.effect_journal(),
            direct.level2.effect_journal()
        );
        assert_eq!(backend.campaign.events(), direct.events());
        assert_eq!(
            encode_reset_v1(&ResetState::from_level2(&backend.campaign.level2)),
            encode_reset_v1(&ResetState::from_level2(&direct.level2))
        );
    }

    #[test]
    fn campaign_backend_matches_direct_session_with_observational_polling() {
        let root = descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let definition = Arc::new(load_mc2_campaign(&root, &catalog).unwrap());
        let mut neutral = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::clone(&definition),
            11,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let mut direct = CampaignSession::open(&root, CampaignSelection::LEVEL2).unwrap();
        // The direct authority mirrors the campaign route's entry binding so
        // the equality below stays discriminating for every later value.
        direct
            .level2_compatibility_mut()
            .select_level2_campaign_entry_stage();
        assert_matches_direct(&neutral, &direct);
        let initial = neutral.snapshot();
        assert_eq!(initial.identity.generation, 1);
        assert_eq!(initial.value.effect_head, 0);
        assert_eq!(initial.value.state_hash, direct.state_hash());
        assert_eq!(initial.value.terrain_hash, direct.terrain_hash());
        assert_eq!(
            initial.value.effect_journal_sha256,
            hex_digest(direct.level2.effect_journal().receipt().as_bytes())
        );

        let command = GameCommand {
            longitudinal: LongitudinalControl::Accelerate,
            ..GameCommand::default()
        };

        // Zero/one/many observational polls before the first neutral step;
        // the direct authority is never polled.
        for _ in 0..3 {
            assert_eq!(neutral.snapshot(), neutral.snapshot());
            let _ = neutral
                .events_after(guard(&neutral), 0, usize::MAX)
                .unwrap();
        }
        let first = neutral.step(guard(&neutral), command).unwrap();
        let direct_evidence = direct.step(campaign_player_command(command), 0);
        assert!(!direct_evidence.is_empty());
        assert_matches_direct(&neutral, &direct);
        assert_eq!(first.value.state_hash, direct.state_hash());
        assert!(first.value.effect_head > 0);

        let page = neutral
            .events_after(guard(&neutral), 0, usize::MAX)
            .unwrap()
            .value;
        let direct_page = direct
            .effects_after(
                EffectCursor {
                    generation: direct.generation(),
                    next_sequence: 0,
                },
                usize::MAX,
            )
            .unwrap();
        assert_eq!(page.events.len(), direct_page.records.len());
        assert!(!page.events.is_empty());
        assert!(!page.has_more);
        assert_eq!(page.next_cursor, page.event_head);
        for (event, record) in page.events.iter().zip(direct_page.records.iter()) {
            assert_eq!(event.sequence, record.sequence + 1);
            let id = event.event_id.as_str();
            for leaked in ["level", "source", "slot", "class", "model"] {
                assert!(!id.contains(leaked), "{id} leaks compatibility vocabulary");
            }
        }

        for _ in 0..3 {
            assert_eq!(neutral.snapshot(), first);
            let _ = neutral
                .events_after(guard(&neutral), 0, usize::MAX)
                .unwrap();
        }
        let second = neutral.step(guard(&neutral), command).unwrap();
        let _ = direct.step(campaign_player_command(command), 0);
        assert_matches_direct(&neutral, &direct);
        assert_eq!(second.value.state_hash, direct.state_hash());
        assert_ne!(second.value, first.value);

        let reset = neutral.reset(guard(&neutral)).unwrap();
        direct.reset();
        assert_matches_direct(&neutral, &direct);
        assert_eq!(reset.identity.generation, 2);
        assert_eq!(reset.value, initial.value);

        let replay = neutral.step(guard(&neutral), command).unwrap();
        let replay_evidence = direct.step(campaign_player_command(command), 0);
        assert_eq!(replay_evidence, direct_evidence);
        assert_matches_direct(&neutral, &direct);
        assert_eq!(replay.value, first.value);
        assert_eq!(replay.identity.generation, 2);
    }

    #[test]
    fn cf14_neutral_backend_derives_level3_selection_from_package() {
        let root = level_three_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let definition = Arc::new(load_mc2_campaign(&root, &catalog).unwrap());
        let mut neutral = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::clone(&definition),
            14,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let mut direct = CampaignSession::open(&root, CampaignSelection::LEVEL3).unwrap();

        assert_eq!(
            neutral.backend().campaign.definition().identity,
            CampaignSelection::LEVEL3
        );
        assert_matches_direct(&neutral, &direct);
        let initial = neutral.snapshot();
        let command = GameCommand::default();
        let stepped = neutral.step(guard(&neutral), command).unwrap();
        let _ = direct.step(campaign_player_command(command), 0);
        assert_matches_direct(&neutral, &direct);
        assert_ne!(stepped.value.state_hash, initial.value.state_hash);
        let reset = neutral.reset(guard(&neutral)).unwrap();
        direct.reset();
        assert_matches_direct(&neutral, &direct);
        assert_eq!(reset.value, initial.value);
    }

    #[test]
    fn campaign_backend_consumes_published_flight_tuning_across_level_packages() {
        let catalog = default_catalog().unwrap();
        for (ordinal, root) in [
            level_one_descriptor_fixture_path(),
            descriptor_fixture_path(),
            level_three_descriptor_fixture_path(),
        ]
        .into_iter()
        .enumerate()
        {
            let base = load_mc2_campaign(&root, &catalog).unwrap();
            let workspace_root = temporary_workspace_path(&format!("campaign-flight-{ordinal}"));
            let store =
                WorkspaceStore::create(&workspace_root, "campaign-flight-authoring", base.clone())
                    .unwrap();
            let mut existing = GameSession::open(
                &Mc2CampaignAdapter,
                Arc::new(base.clone()),
                40 + u64::try_from(ordinal).unwrap(),
                Mc2CampaignInputs::new(&root),
            )
            .unwrap();
            let initial = existing.snapshot();

            let speed_16 = publish_player_max_forward_speed(
                &store,
                &catalog,
                &base,
                &format!("publish-speed-16-{ordinal}"),
                16,
            );
            let speed_48 = publish_player_max_forward_speed(
                &store,
                &catalog,
                &speed_16,
                &format!("publish-speed-48-{ordinal}"),
                48,
            );
            if ordinal == 0 {
                let before = store.state_bytes().unwrap();
                let unsupported = ChangeSetBuilder::new(
                    "campaign-flight-authoring",
                    &speed_48,
                    "unsupported-campaign-fireball-cost",
                    RequestedTiming::SessionReset,
                )
                .set_integer(IntegerParameter::FireballManaCost, 200)
                .build()
                .unwrap();
                let rejected = store.validate(&catalog, unsupported).unwrap();
                assert!(!rejected.accepted);
                assert_eq!(rejected.diagnostics[0].code, "DEPENDENCY_FAILED");
                assert_eq!(store.state_bytes().unwrap(), before);
            }

            assert_eq!(existing.identity().definition_hash, base.definition_hash());
            assert_eq!(existing.identity().generation, 1);
            let accelerate = GameCommand {
                longitudinal: LongitudinalControl::Accelerate,
                ..GameCommand::default()
            };
            for _ in 0..4 {
                existing.step(guard(&existing), accelerate).unwrap();
            }
            assert_eq!(existing.snapshot().value.player_motion.target_speed, 64);

            let reopened = existing
                .reopen(guard(&existing), Arc::new(speed_16.clone()))
                .unwrap();
            assert_eq!(reopened.identity.generation, 2);
            assert_eq!(reopened.identity.definition_revision, 2);
            assert_eq!(reopened.value, initial.value);
            for _ in 0..4 {
                existing.step(guard(&existing), accelerate).unwrap();
            }
            let capped_16 = existing.snapshot();
            assert_eq!(capped_16.value.player_motion.target_speed, 16);
            assert_eq!(capped_16.value.player_motion.actual_speed, 16);

            let mut authored_48 = GameSession::open(
                &Mc2CampaignAdapter,
                Arc::new(speed_48),
                50 + u64::try_from(ordinal).unwrap(),
                Mc2CampaignInputs::new(&root),
            )
            .unwrap();
            for _ in 0..4 {
                authored_48.step(guard(&authored_48), accelerate).unwrap();
            }
            let capped_48 = authored_48.snapshot();
            assert_eq!(capped_48.value.player_motion.target_speed, 48);
            assert_eq!(capped_48.value.player_motion.actual_speed, 48);
            assert_ne!(capped_48.value.player_pose, capped_16.value.player_pose);

            authored_48.reset(guard(&authored_48)).unwrap();
            for _ in 0..4 {
                authored_48.step(guard(&authored_48), accelerate).unwrap();
            }
            assert_eq!(authored_48.snapshot().value, capped_48.value);

            std::fs::remove_dir_all(&workspace_root).unwrap();
        }
    }

    #[test]
    fn campaign_backend_moves_real_start_trigger_only_on_reopen_and_runs_ordinary_group_chain() {
        const TRIGGER_SLOT: usize = 48;
        const GROUP_ONE_ROWS: &[u16] = &[
            31, 99, 104, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 168, 169, 170, 171, 172,
            173, 174, 175,
        ];
        let root = level_one_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let start_trigger =
            sim_definition::adapter::stable_entity_id("switch.start_area_exit").unwrap();
        let fixed_trigger =
            sim_definition::adapter::stable_entity_id("switch.area_exit_trigger.at_189_212")
                .unwrap();
        assert_eq!(
            base.level_definition()
                .unwrap()
                .entity(&start_trigger)
                .unwrap()
                .position(),
            TilePosition { x: 74, y: 212 }
        );
        assert_eq!(
            base.level_definition()
                .unwrap()
                .entity(&fixed_trigger)
                .unwrap()
                .position(),
            TilePosition { x: 189, y: 212 }
        );

        let workspace_root = temporary_workspace_path("campaign-placement");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-placement-authoring",
            base.clone(),
        )
        .unwrap();
        let mut existing = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            70,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let base_state_hash = existing.backend().compatibility_campaign().state_hash();

        let before_rejection = store.state_bytes().unwrap();
        let unsupported = ChangeSetBuilder::new(
            "campaign-placement-authoring",
            &base,
            "reject-read-only-trigger",
            RequestedTiming::SessionReset,
        )
        .move_entity(fixed_trigger.clone(), TilePosition { x: 188, y: 212 })
        .build()
        .unwrap();
        let rejected = store.validate(&catalog, unsupported).unwrap();
        assert!(!rejected.accepted);
        assert_eq!(rejected.diagnostics[0].code, "DEPENDENCY_FAILED");
        assert_eq!(store.state_bytes().unwrap(), before_rejection);

        let moved_tile = TilePosition { x: 150, y: 100 };
        let moved =
            publish_start_trigger_move(&store, &catalog, &base, "move-start-trigger", moved_tile);
        assert_eq!(existing.identity().definition_hash, base.definition_hash());
        assert_eq!(
            existing
                .backend()
                .compatibility_campaign()
                .level2_compatibility()
                .entities()[TRIGGER_SLOT]
                .position,
            crate::Position {
                x: 19_072,
                y: 54_400,
                z: 4_896,
            }
        );
        for _ in 0..8 {
            existing
                .step(guard(&existing), GameCommand::default())
                .unwrap();
        }
        assert!(
            !existing
                .backend()
                .compatibility_campaign()
                .events()
                .iter()
                .any(|event| matches!(
                    event,
                    Level2DispatchEvent::AuthoredGroupActivated { group: 1, .. }
                ))
        );

        let reopened = existing
            .reopen(guard(&existing), Arc::new(moved.clone()))
            .unwrap();
        assert_eq!(reopened.identity.generation, 2);
        assert_eq!(reopened.identity.definition_revision, 2);
        assert_ne!(moved.definition_hash(), base.definition_hash());
        let moved_reset_hash = existing.backend().compatibility_campaign().state_hash();
        assert_ne!(moved_reset_hash, base_state_hash);
        let active_campaign = existing.backend().compatibility_campaign();
        assert_eq!(
            active_campaign
                .definition()
                .level_definition()
                .entity(&start_trigger)
                .unwrap()
                .position(),
            moved_tile
        );
        assert_eq!(
            active_campaign
                .level2_compatibility()
                .semantic_level_definition()
                .unwrap()
                .entity(&start_trigger)
                .unwrap()
                .position(),
            moved_tile
        );
        let moved_source = &active_campaign.level2_compatibility().entities()[TRIGGER_SLOT];
        let moved_position = crate::Position {
            x: 38_528,
            y: 25_728,
            z: 4_896,
        };
        assert_eq!(moved_source.position, moved_position);
        assert_eq!(moved_source.phase, 49);
        let map_chains = active_campaign
            .level2_compatibility()
            .economy()
            .map_chains();
        assert!(map_chains.is_linked(TRIGGER_SLOT as u16));
        assert_eq!(
            map_chains.tile_of(TRIGGER_SLOT as u16),
            u16::from_le_bytes([150, 100])
        );
        assert!(
            !map_chains
                .entity_chain(74, 212)
                .contains(&(TRIGGER_SLOT as u16))
        );
        assert!(
            map_chains
                .entity_chain(150, 100)
                .contains(&(TRIGGER_SLOT as u16))
        );
        assert_eq!(
            active_campaign
                .level2
                .session
                .state
                .live
                .level2_imported_switch_roots
                .iter()
                .find(|root| root.source_slot == TRIGGER_SLOT as u16)
                .unwrap()
                .position,
            moved_position
        );
        assert_eq!(
            moved
                .level_definition()
                .unwrap()
                .entity(&fixed_trigger)
                .unwrap()
                .position(),
            TilePosition { x: 189, y: 212 }
        );

        for _ in 0..7 {
            existing
                .step(guard(&existing), GameCommand::default())
                .unwrap();
            assert!(
                !existing
                    .backend()
                    .compatibility_campaign()
                    .events()
                    .iter()
                    .any(|event| matches!(
                        event,
                        Level2DispatchEvent::AuthoredGroupActivated { group: 1, .. }
                    ))
            );
        }
        existing
            .step(guard(&existing), GameCommand::default())
            .unwrap();
        let campaign = existing.backend().compatibility_campaign();
        let activations = campaign
            .events()
            .iter()
            .filter_map(|event| match event {
                Level2DispatchEvent::AuthoredGroupActivated {
                    source_slot,
                    group: 1,
                    authored_indices,
                    ..
                } => Some((*source_slot, authored_indices.as_ref())),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(activations, vec![(TRIGGER_SLOT as u16, GROUP_ONE_ROWS)]);
        assert!(
            !campaign
                .events()
                .iter()
                .any(|event| matches!(event, Level2DispatchEvent::Error { .. }))
        );
        let hidden = &campaign.level2_compatibility().entities()[TRIGGER_SLOT];
        assert!(hidden.active);
        assert!(hidden.drawing_disabled());
        assert_eq!(hidden.phase, 57);

        existing
            .step(guard(&existing), GameCommand::default())
            .unwrap();
        let reclaimed = &existing
            .backend()
            .compatibility_campaign()
            .level2_compatibility()
            .entities()[TRIGGER_SLOT];
        assert!(!reclaimed.active);
        assert!(
            !existing
                .backend()
                .compatibility_campaign()
                .level2_compatibility()
                .economy()
                .map_chains()
                .is_linked(TRIGGER_SLOT as u16)
        );

        existing.reset(guard(&existing)).unwrap();
        let reset_campaign = existing.backend().compatibility_campaign();
        assert_eq!(reset_campaign.state_hash(), moved_reset_hash);
        let reset_trigger = &reset_campaign.level2_compatibility().entities()[TRIGGER_SLOT];
        assert!(reset_trigger.active);
        assert!(!reset_trigger.drawing_disabled());
        assert_eq!(reset_trigger.phase, 49);
        assert_eq!(reset_trigger.position, moved_position);
        assert_eq!(
            reset_campaign
                .level2
                .session
                .state
                .live
                .level2_imported_switch_roots
                .iter()
                .find(|root| root.source_slot == TRIGGER_SLOT as u16)
                .unwrap()
                .position,
            moved_position
        );
        for _ in 0..8 {
            existing
                .step(guard(&existing), GameCommand::default())
                .unwrap();
        }
        assert!(
            existing
                .backend()
                .compatibility_campaign()
                .events()
                .iter()
                .any(|event| matches!(
                    event,
                    Level2DispatchEvent::AuthoredGroupActivated {
                        source_slot: 48,
                        group: 1,
                        authored_indices,
                        ..
                    } if authored_indices.as_ref() == GROUP_ONE_ROWS
                ))
        );

        std::fs::remove_dir_all(&workspace_root).unwrap();
    }

    #[test]
    fn campaign_backend_reach_location_override_drives_ordinary_objective_after_reopen_and_reset() {
        const STAGE: usize = 3;
        let root = descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let objective_id = "objective.reach.payahandra_ruins";
        let original = ObjectiveCondition::ReachLocation(TilePosition { x: 154, y: 229 });
        let authored = ObjectiveCondition::ReachLocation(TilePosition { x: 120, y: 120 });
        let workspace_root = temporary_workspace_path("campaign-objective-authoring");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-objective-authoring",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            71,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        assert_eq!(
            session
                .backend()
                .campaign
                .level2
                .session
                .state
                .live
                .core
                .runtime
                .objectives[0]
                .stage_selectors[STAGE],
            1
        );

        let published = publish_reach_objective(
            &store,
            &catalog,
            &base,
            "campaign-objective-authoring",
            objective_id,
            original,
            authored,
        );
        let player_slot = session
            .backend()
            .campaign
            .level2_compatibility()
            .state
            .entity_index;
        let z = session
            .backend()
            .campaign
            .level2_compatibility()
            .state
            .position
            .z;
        session
            .backend_mut()
            .campaign
            .level2_compatibility_mut()
            .set_level2_entity_position(
                player_slot,
                crate::Position {
                    x: 120 << 8,
                    y: 120 << 8,
                    z,
                },
            )
            .unwrap();
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        assert_eq!(
            session
                .backend()
                .campaign
                .level2
                .session
                .state
                .live
                .core
                .runtime
                .objectives[0]
                .stage_selectors[STAGE],
            1,
            "the already-open base session must not adopt the published waypoint"
        );

        session
            .reopen(guard(&session), Arc::new(published.clone()))
            .unwrap();
        let objective = sim_definition::adapter::stable_objective_id(objective_id).unwrap();
        assert_eq!(
            session
                .backend()
                .campaign
                .definition()
                .level_definition()
                .objective(&objective)
                .unwrap()
                .condition(),
            authored
        );
        assert_eq!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .semantic_level_definition()
                .unwrap()
                .objective(&objective)
                .unwrap()
                .condition(),
            authored
        );
        let stage = &session
            .backend()
            .campaign
            .level2
            .session
            .state
            .live
            .core
            .runtime
            .stages[STAGE];
        assert_eq!((stage.axis_x, stage.axis_y), (-26_112, -6_912));

        for (tile, expected) in [
            (TilePosition { x: 154, y: 229 }, 1),
            (TilePosition { x: 120, y: 120 }, 2),
        ] {
            let player_slot = session
                .backend()
                .campaign
                .level2_compatibility()
                .state
                .entity_index;
            session
                .backend_mut()
                .campaign
                .level2_compatibility_mut()
                .set_level2_entity_position(
                    player_slot,
                    crate::Position {
                        x: tile.x << 8,
                        y: tile.y << 8,
                        z,
                    },
                )
                .unwrap();
            session
                .step(guard(&session), GameCommand::default())
                .unwrap();
            assert_eq!(
                session
                    .backend()
                    .campaign
                    .level2
                    .session
                    .state
                    .live
                    .core
                    .runtime
                    .objectives[0]
                    .stage_selectors[STAGE],
                expected
            );
        }

        session.reset(guard(&session)).unwrap();
        assert_eq!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .semantic_level_definition()
                .unwrap()
                .objective(&objective)
                .unwrap()
                .condition(),
            authored
        );
        let player_slot = session
            .backend()
            .campaign
            .level2_compatibility()
            .state
            .entity_index;
        session
            .backend_mut()
            .campaign
            .level2_compatibility_mut()
            .set_level2_entity_position(
                player_slot,
                crate::Position {
                    x: 120 << 8,
                    y: 120 << 8,
                    z,
                },
            )
            .unwrap();
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        assert_eq!(
            session
                .backend()
                .campaign
                .level2
                .session
                .state
                .live
                .core
                .runtime
                .objectives[0]
                .stage_selectors[STAGE],
            2
        );
        std::fs::remove_dir_all(workspace_root).unwrap();
    }

    #[test]
    fn campaign_backend_countdown_override_is_instance_bound_and_runtime_consumed_after_reopen() {
        const WORM_ID: &str = "mechanic.absence_countdown.worm.at_134_98";
        const ARCHER_ID: &str = "mechanic.absence_countdown.archer.at_144_255";
        const GROUP34_ROWS: [u16; 3] = [309, 310, 311];

        let root = level_one_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let workspace_root = temporary_workspace_path("campaign-countdown-authoring");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-countdown-authoring",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            72,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let published = publish_countdown_delay(
            &store,
            &catalog,
            &base,
            "campaign-countdown-authoring",
            WORM_ID,
            16,
            8,
        );

        {
            let compatibility = session.backend_mut().campaign.level2_compatibility_mut();
            compatibility
                .activate_authored_group_prerequisite_for_test(4)
                .unwrap();
            assert!(
                compatibility
                    .economy_mut()
                    .retire_authored_multipart_prerequisite_for_test(307)
            );
        }
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        assert_eq!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .economy()
                .authored_switch_producers()
                .iter()
                .find(|source| source.authored_index == 308)
                .unwrap()
                .dword_0x10,
            16,
            "publishing must not mutate the already-open base session"
        );

        session
            .reopen(guard(&session), Arc::new(published.clone()))
            .unwrap();
        let semantic_level = session
            .backend()
            .campaign
            .level2_compatibility()
            .semantic_level_definition()
            .unwrap();
        for (id, expected) in [(WORM_ID, 8), (ARCHER_ID, 16)] {
            let id = sim_definition::adapter::stable_entity_id(id).unwrap();
            assert_eq!(
                semantic_level
                    .entity(&id)
                    .unwrap()
                    .absence_countdown()
                    .unwrap()
                    .arming_ticks(),
                expected,
                "the operation must be bound to one stable mechanic instance"
            );
        }

        let countdown_slot = {
            let compatibility = session.backend_mut().campaign.level2_compatibility_mut();
            compatibility
                .activate_authored_group_prerequisite_for_test(4)
                .unwrap();
            compatibility
                .economy()
                .authored_switch_producers()
                .iter()
                .find(|source| source.authored_index == 308)
                .unwrap()
                .source_entity_index
        };
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        assert_eq!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .economy()
                .authored_switch_producers()
                .iter()
                .find(|source| source.source_entity_index == countdown_slot)
                .unwrap()
                .dword_0x10,
            0,
            "the live Worm must freeze a zero timer; configuration is not constructor state"
        );
        assert!(
            session
                .backend_mut()
                .campaign
                .level2_compatibility_mut()
                .economy_mut()
                .retire_authored_multipart_prerequisite_for_test(307)
        );
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        assert_eq!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .economy()
                .authored_switch_producers()
                .iter()
                .find(|source| source.source_entity_index == countdown_slot)
                .unwrap()
                .dword_0x10,
            8
        );
        for expected in (1..8).rev() {
            session
                .step(guard(&session), GameCommand::default())
                .unwrap();
            assert_eq!(
                session
                    .backend()
                    .campaign
                    .level2_compatibility()
                    .economy()
                    .authored_switch_producers()
                    .iter()
                    .find(|source| source.source_entity_index == countdown_slot)
                    .unwrap()
                    .dword_0x10,
                expected
            );
        }
        let journal_start = session
            .backend()
            .campaign
            .level2_compatibility()
            .effect_journal()
            .records()
            .len();
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        let compatibility = session.backend().campaign.level2_compatibility();
        let records = &compatibility.effect_journal().records()[journal_start..];
        let sound = records
            .iter()
            .position(|record| {
                matches!(
                    record.effect,
                    crate::campaign::journal::CampaignEffect::Flight(FlightEvent::PrepareSound {
                        entity_index,
                        sound_id: 41,
                        ..
                    }) if entity_index == countdown_slot
                )
            })
            .unwrap();
        let activation = records
            .iter()
            .position(|record| {
                matches!(
                    &record.effect,
                    crate::campaign::journal::CampaignEffect::Dispatch(
                        Level2DispatchEvent::AuthoredGroupActivated {
                            source_slot,
                            group: 34,
                            authored_indices,
                            ..
                        }
                    ) if *source_slot == countdown_slot && authored_indices.as_ref() == GROUP34_ROWS
                )
            })
            .unwrap();
        assert!(sound < activation);
        assert!(
            compatibility
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Level2DispatchEvent::Error { .. }))
        );
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        assert!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .economy()
                .authored_switch_producers()
                .iter()
                .all(|source| source.source_entity_index != countdown_slot)
        );

        session.reset(guard(&session)).unwrap();
        {
            let compatibility = session.backend_mut().campaign.level2_compatibility_mut();
            compatibility
                .activate_authored_group_prerequisite_for_test(4)
                .unwrap();
            assert!(
                compatibility
                    .economy_mut()
                    .retire_authored_multipart_prerequisite_for_test(307)
            );
        }
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        assert_eq!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .economy()
                .authored_switch_producers()
                .iter()
                .find(|source| source.authored_index == 308)
                .unwrap()
                .dword_0x10,
            8,
            "reset must reproduce the authored definition without pre-loading live state"
        );

        let level_two_root = descriptor_fixture_path();
        let level_two = load_mc2_campaign(&level_two_root, &catalog).unwrap();
        let rejected_root = temporary_workspace_path("campaign-countdown-l2-rejection");
        let rejected_store = WorkspaceStore::create(
            &rejected_root,
            "campaign-countdown-l2-rejection",
            level_two.clone(),
        )
        .unwrap();
        let before = rejected_store.state_bytes().unwrap();
        let request = ChangeSetBuilder::new(
            "campaign-countdown-l2-rejection",
            &level_two,
            "reject-absent-countdown",
            RequestedTiming::SessionReset,
        )
        .set_mechanic_countdown(
            sim_definition::adapter::stable_entity_id(WORM_ID).unwrap(),
            8,
        )
        .build()
        .unwrap();
        let validation = rejected_store.validate(&catalog, request).unwrap();
        assert!(!validation.accepted);
        assert_eq!(rejected_store.state_bytes().unwrap(), before);

        std::fs::remove_dir_all(workspace_root).unwrap();
        std::fs::remove_dir_all(rejected_root).unwrap();
        // No-Claim: the test-only prerequisite seam does not prove Worm
        // combat/death. This gate owns typed publication, per-instance binding,
        // ordinary countdown consumption, group output, reclaim and reset.
    }

    #[test]
    fn campaign_backend_countdown_override_reaches_imported_level_three_carrier() {
        const WORM_ID: &str = "mechanic.absence_countdown.worm.at_160_106";
        const COUNTDOWN_SLOT: usize = 247;
        const GROUP11_ROWS: [u16; 10] = [247, 248, 249, 250, 251, 252, 253, 254, 255, 256];

        fn countdown_timer(source: &crate::native_world::EntitySnapshot) -> i32 {
            i32::from_le_bytes(source.record_bytes()[20..24].try_into().unwrap())
        }

        let root = level_three_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let workspace_root = temporary_workspace_path("campaign-l3-countdown-authoring");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-l3-countdown-authoring",
            base.clone(),
        )
        .unwrap();
        let published = publish_countdown_delay(
            &store,
            &catalog,
            &base,
            "campaign-l3-countdown-authoring",
            WORM_ID,
            16,
            2,
        );
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base),
            73,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        assert_eq!(
            session
                .backend_mut()
                .campaign
                .level2_compatibility_mut()
                .hide_imported_stage_worm_roots_for_test(),
            5
        );

        for expected in [2, 1] {
            session
                .step(guard(&session), GameCommand::default())
                .unwrap();
            assert_eq!(
                countdown_timer(
                    &session.backend().campaign.level2_compatibility().entities()[COUNTDOWN_SLOT],
                ),
                expected
            );
        }
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        let compatibility = session.backend().campaign.level2_compatibility();
        let source = &compatibility.entities()[COUNTDOWN_SLOT];
        assert!(source.active && source.drawing_disabled());
        assert!(compatibility.dispatch_events().iter().any(|event| matches!(
            event,
            Level2DispatchEvent::AuthoredGroupActivated {
                source_slot: 247,
                group: 11,
                authored_indices,
                ..
            } if authored_indices.as_ref() == GROUP11_ROWS
        )));
        assert!(
            compatibility
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Level2DispatchEvent::Error { .. }))
        );
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        assert!(
            !session.backend().campaign.level2_compatibility().entities()[COUNTDOWN_SLOT].active
        );
        session.reset(guard(&session)).unwrap();
        let reset = session.backend().campaign.level2_compatibility();
        assert_eq!(countdown_timer(&reset.entities()[COUNTDOWN_SLOT]), 0);
        let entity = sim_definition::adapter::stable_entity_id(WORM_ID).unwrap();
        assert_eq!(
            reset
                .semantic_level_definition()
                .unwrap()
                .entity(&entity)
                .unwrap()
                .absence_countdown()
                .unwrap()
                .arming_ticks(),
            2
        );
        std::fs::remove_dir_all(workspace_root).unwrap();
        // No-Claim: this imports one package-present countdown configuration;
        // it does not establish Worm combat or a general reset-row editor.
    }

    #[test]
    fn campaign_backend_rejects_deviation_substitution_and_stale_guards() {
        let root = descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let fireball_cost_id = SemanticId::parse("spell.fireball.mana_cost").unwrap();
        let deviated =
            with_replacement(&base, &fireball_cost_id, SemanticValue::Integer(200), 2).unwrap();

        let error = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(deviated.clone()),
            21,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            crate::game_session::OpenSessionError::Adapter(
                CampaignBackendError::UnappliedSemanticValue { .. }
            )
        ));

        let mut neutral = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            22,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let before = neutral.snapshot();

        let error = neutral
            .reopen(guard(&neutral), Arc::new(deviated))
            .unwrap_err();
        assert!(matches!(
            error,
            crate::game_session::GameSessionError::Backend(
                CampaignBackendError::UnappliedSemanticValue { .. }
            )
        ));
        assert_eq!(neutral.snapshot(), before);
        assert_eq!(neutral.identity().generation, 1);
        assert_eq!(neutral.identity().definition_hash, base.definition_hash());

        let synthetic = GameDefinition::from_package(
            GamePackage::load(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/custom-synthetic-v1"),
            )
            .unwrap(),
            &engine_capabilities().unwrap(),
            &catalog,
        )
        .unwrap();
        let error = neutral
            .reopen(guard(&neutral), Arc::new(synthetic))
            .unwrap_err();
        assert!(matches!(
            error,
            crate::game_session::GameSessionError::Backend(
                CampaignBackendError::PackageSubstitution
            )
        ));
        assert_eq!(neutral.snapshot(), before);

        let stale = SessionGuard {
            expected_tick: 7,
            ..guard(&neutral)
        };
        let error = neutral.step(stale, GameCommand::default()).unwrap_err();
        assert!(matches!(
            error,
            crate::game_session::GameSessionError::Guard(_)
        ));
        assert_eq!(neutral.snapshot(), before);

        let reissued = reissue_at_revision(&base, 2).unwrap();
        let reopened = neutral
            .reopen(guard(&neutral), Arc::new(reissued.clone()))
            .unwrap();
        assert_eq!(reopened.identity.generation, 2);
        assert_eq!(reopened.identity.definition_revision, 2);
        assert_eq!(
            reopened.identity.definition_hash,
            reissued.definition_hash()
        );
        assert_eq!(reopened.value.state_hash, before.value.state_hash);
        assert_eq!(reopened.value.terrain_hash, before.value.terrain_hash);
    }

    #[test]
    fn campaign_backend_pages_effects_with_bounded_cursors() {
        let root = descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let definition = Arc::new(load_mc2_campaign(&root, &catalog).unwrap());
        let mut neutral = GameSession::open(
            &Mc2CampaignAdapter,
            definition,
            31,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        neutral
            .step(guard(&neutral), GameCommand::default())
            .unwrap();

        let full = neutral
            .events_after(guard(&neutral), 0, usize::MAX)
            .unwrap()
            .value;
        assert!(!full.events.is_empty());
        assert!(!full.has_more);
        assert_eq!(full.next_cursor, full.event_head);
        assert!(
            full.events
                .windows(2)
                .all(|pair| pair[1].sequence == pair[0].sequence + 1)
        );
        assert_eq!(full.events.first().unwrap().sequence, 1);

        let mut collected = Vec::new();
        let mut cursor = 0;
        loop {
            let page = neutral
                .events_after(guard(&neutral), cursor, 3)
                .unwrap()
                .value;
            assert_eq!(page.requested_cursor, cursor);
            collected.extend(page.events);
            if !page.has_more {
                break;
            }
            cursor = page.next_cursor;
        }
        assert_eq!(collected, full.events);

        let empty = neutral.events_after(guard(&neutral), 0, 0).unwrap().value;
        assert!(empty.events.is_empty());
        assert!(empty.has_more);
        assert_eq!(empty.next_cursor, 0);

        let drained = neutral
            .events_after(guard(&neutral), full.event_head, 1)
            .unwrap()
            .value;
        assert!(drained.events.is_empty());
        assert!(!drained.has_more);

        let error = neutral
            .events_after(guard(&neutral), full.event_head + 1, 1)
            .unwrap_err();
        assert!(matches!(
            error,
            crate::game_session::GameSessionError::Backend(
                CampaignBackendError::CursorBeyondHead { .. }
            )
        ));

        let error = neutral.audio_for_event(guard(&neutral), 1).unwrap_err();
        assert!(matches!(
            error,
            crate::game_session::GameSessionError::Backend(CampaignBackendError::NoAudioSidecar {
                event_sequence: 1
            })
        ));
    }

    #[test]
    fn campaign_command_maps_inside_the_adapter() {
        let mapped = campaign_player_command(GameCommand {
            longitudinal: LongitudinalControl::Decelerate,
            lateral: LateralControl::Positive,
            roll_axis: -32,
            pitch_axis: 24,
            cast_left: true,
            cast_right: false,
            cast_selected: true,
            look_yaw: -17,
            look_pitch: 23,
        });
        assert_eq!(mapped.opcode, 0);
        assert_eq!(mapped.argument_1, 0);
        assert_eq!(mapped.argument_2, 0);
        assert_eq!(mapped.flags.cast_unsigned(), 0x02 | 0x08 | 0x10 | 0x40);
        assert_eq!(mapped.roll, -32);
        assert_eq!(mapped.pitch, 24);
        assert_eq!(mapped.argument_6.cast_signed(), -17);
        assert_eq!(mapped.argument_8.cast_signed(), 23);
        assert_eq!(
            campaign_player_command(GameCommand::default()),
            PlayerCommand::default()
        );
    }
}
