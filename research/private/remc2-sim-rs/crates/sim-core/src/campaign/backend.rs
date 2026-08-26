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

use crate::{
    FlightEvent, PlayerCommand,
    campaign::{
        CampaignOpenError, CampaignSession,
        dispatch::CAMPAIGN_BEHAVIOR_REGISTRY,
        journal::{CampaignEffect, EffectCursor, EffectPageError, EffectRecord},
        spec::{CampaignDefinition as CampaignDescriptor, CampaignSelection},
    },
    game_definition::{
        ArtifactKind, ArtifactNode, DefinitionError, DefinitionErrorCode, GameDefinition,
        GamePackage, PackageProvenance, RequiredCapability, SemanticCatalog, SemanticId,
        default_catalog, engine_capabilities,
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
        DefinitionError::new(
            DefinitionErrorCode::InvalidSchema,
            "mc2_campaign.package",
            format!("cannot read campaign selection: {error}"),
        )
    })?;
    let descriptor =
        CampaignDescriptor::preflight(package_root, selection, &CAMPAIGN_BEHAVIOR_REGISTRY)
            .map_err(|error| {
                DefinitionError::new(
                    DefinitionErrorCode::InvalidSchema,
                    "mc2_campaign.package",
                    format!("campaign descriptor preflight failed: {error}"),
                )
            })?;
    let export_sha256 = descriptor.owner_export_package_sha256().map_err(|error| {
        DefinitionError::new(
            DefinitionErrorCode::HashMismatch,
            "mc2_campaign.owner_export",
            format!("cannot reconstruct wrapped owner export identity: {error}"),
        )
    })?;
    let (package_bytes, envelope_bytes) = campaign_package_bytes(package_root)?;
    let artifacts = vec![
        ArtifactNode::adapter(
            "campaign.owner_export",
            ArtifactKind::World,
            "campaign/owner-export",
            &export_sha256,
            package_bytes - envelope_bytes,
            &[],
        )?,
        ArtifactNode::adapter(
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
    let package = GamePackage::materialize(
        "mc2.campaign",
        1,
        requirements,
        artifacts,
        Vec::new(),
        PackageProvenance::owner_derived(&descriptor.package_sha256)?,
    )?;
    GameDefinition::from_package(package, &engine_capabilities()?, catalog)
        .map(|definition| definition.with_level_definition(descriptor.level_definition().clone()))
}

fn campaign_package_bytes(root: &Path) -> crate::game_definition::Result<(u64, u64)> {
    let mut total = 0_u64;
    let mut envelope = 0_u64;
    for entry in fs::read_dir(root).map_err(|error| {
        DefinitionError::new(
            DefinitionErrorCode::Io,
            "mc2_campaign.package",
            format!("cannot enumerate campaign package: {error}"),
        )
    })? {
        let entry = entry.map_err(|error| {
            DefinitionError::new(
                DefinitionErrorCode::Io,
                "mc2_campaign.package",
                format!("cannot inspect campaign package: {error}"),
            )
        })?;
        let metadata = entry.metadata().map_err(|error| {
            DefinitionError::new(
                DefinitionErrorCode::Io,
                "mc2_campaign.package",
                format!("cannot inspect campaign package member: {error}"),
            )
        })?;
        let file_type = entry.file_type().map_err(|error| {
            DefinitionError::new(
                DefinitionErrorCode::Io,
                "mc2_campaign.package",
                error.to_string(),
            )
        })?;
        if !metadata.is_file() || file_type.is_symlink() {
            return Err(DefinitionError::new(
                DefinitionErrorCode::PathEscape,
                entry.path().display().to_string(),
                "campaign package contains a non-regular or symlink member",
            ));
        }
        total = total.checked_add(metadata.len()).ok_or_else(|| {
            DefinitionError::new(
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
        require_catalog_defaults(definition, &catalog)?;
        let package_base = load_mc2_campaign(&input.package_root, &catalog)
            .map_err(CampaignBackendError::Definition)?;
        definition
            .verify_semantic_variant_of(&package_base, &catalog)
            .map_err(CampaignBackendError::Definition)?;
        let selection = CampaignDescriptor::declared_selection(&input.package_root)
            .map_err(|error| CampaignBackendError::Open(CampaignOpenError::Preflight(error)))?;
        let mut campaign = match &input.owner_cache {
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
            campaign
                .level2_compatibility_mut()
                .select_level2_campaign_entry_stage();
        }
        Ok(Mc2CampaignBackend {
            campaign,
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
        let _evidence = candidate.step(campaign_player_command(command), CAMPAIGN_SPEED_INDEX);
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
        definition
            .verify_semantic_variant_of(&self.package_base, &catalog)
            .map_err(CampaignBackendError::Definition)?;
        require_catalog_defaults(definition, &catalog)?;
        self.campaign.reset();
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
    use crate::game_definition::SemanticValue;
    use crate::game_session::{GameSession, SessionGuard};
    use crate::level_definition::{
        ActivationGroupId, CreatureKind, LevelDefinition, LevelEntityDefinition, LevelEntityId,
        LevelEntityKind, LevelObjectiveDefinition, MechanicKind, ObjectiveCondition, SceneryKind,
        SpellKind, TilePosition,
    };
    use std::{collections::BTreeSet, path::PathBuf, sync::Arc};

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

    fn assert_package_content(
        root: &Path,
        expected_creatures: &[(CreatureKind, usize)],
        expected_spells: &[(SpellKind, TilePosition, Option<u16>)],
        expected_scenery: &[(SceneryKind, usize)],
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
                    | LevelEntityKind::Scenery(_) => None,
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
                (SpellKind::Speed, TilePosition { x: 194, y: 213 }, Some(1)),
                (
                    SpellKind::Possession,
                    TilePosition { x: 202, y: 212 },
                    Some(11),
                ),
            ],
            &[(SceneryKind::Tree, 18), (SceneryKind::Stone, 1)],
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
        assert_package_content(
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
                (
                    SpellKind::Earthquake,
                    TilePosition { x: 138, y: 219 },
                    Some(22),
                ),
                (
                    SpellKind::RubberBand,
                    TilePosition { x: 138, y: 237 },
                    Some(27),
                ),
                (SpellKind::Invisibility, TilePosition { x: 1, y: 152 }, None),
                (SpellKind::Possession, TilePosition { x: 167, y: 59 }, None),
            ],
            &[(SceneryKind::Tree, 12), (SceneryKind::Stone, 1)],
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
        assert_package_content(
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
                (SpellKind::Earthquake, TilePosition { x: 85, y: 85 }, None),
                (SpellKind::RubberBand, TilePosition { x: 136, y: 85 }, None),
                (SpellKind::Shield, TilePosition { x: 110, y: 109 }, Some(4)),
            ],
            &[(SceneryKind::Tree, 37), (SceneryKind::Stone, 12)],
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
        let level = definition
            .level_definition()
            .expect("the campaign package exposes semantic level content");

        let swapped = level
            .entities()
            .iter()
            .enumerate()
            .map(|(ordinal, entity)| match entity.kind() {
                LevelEntityKind::Creature(kind) => {
                    let wrong_kind = match kind {
                        CreatureKind::Archer => CreatureKind::Firefly,
                        _ => CreatureKind::Archer,
                    };
                    LevelEntityDefinition::new(
                        LevelEntityId::creature(
                            wrong_kind,
                            u16::try_from(ordinal)
                                .expect("Level 1 semantic entity ordinal fits u16"),
                        ),
                        LevelEntityKind::Creature(wrong_kind),
                        entity.position(),
                        entity.activation_group(),
                    )
                }
                LevelEntityKind::SpellPickup(_)
                | LevelEntityKind::Scenery(_)
                | LevelEntityKind::Mechanic(_) => entity.clone(),
            })
            .collect();
        let activation_groups = level.activation_groups().collect::<Vec<_>>();
        let tampered = definition.clone().with_level_definition(
            LevelDefinition::new(
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
        wrong_objectives[0] =
            LevelObjectiveDefinition::new(ObjectiveCondition::CastleManaPercent(99));
        let tampered = definition.clone().with_level_definition(
            LevelDefinition::new(
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

        let alias_package = GamePackage::materialize(
            "mc2.campaign.alias",
            definition.package_version(),
            definition.required_capabilities().to_vec(),
            definition.artifacts().to_vec(),
            Vec::new(),
            definition.provenance().clone(),
        )
        .unwrap();
        let alias_definition =
            GameDefinition::from_package(alias_package, &engine_capabilities().unwrap(), &catalog)
                .unwrap()
                .with_level_definition(level.clone());
        assert_eq!(alias_definition.level_definition(), Some(level));
        let error = Mc2CampaignAdapter
            .open_backend(&alias_definition, Mc2CampaignInputs::new(&root))
            .expect_err("matching semantic content must not bypass package identity");
        assert!(matches!(error, CampaignBackendError::Definition(_)));

        Mc2CampaignAdapter
            .open_backend(&definition, Mc2CampaignInputs::new(&root))
            .expect("the package-derived semantic definition opens the shared campaign engine");
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
    fn campaign_backend_rejects_deviation_substitution_and_stale_guards() {
        let root = descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let speed_id = SemanticId::parse("flight.player.max_forward_speed").unwrap();
        let deviated = base
            .with_replacement(&speed_id, SemanticValue::Integer(16), 2)
            .unwrap();

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

        let reissued = base.reissue_at_revision(2).unwrap();
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
