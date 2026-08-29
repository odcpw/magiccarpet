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
    collections::BTreeMap,
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sim_definition::adapter::{
    artifact_node, definition_error, game_package_with_payloads, owner_provenance,
    with_level_and_runtime_flow,
};
use sim_definition::runtime_flow::{
    RuntimeFlowDefinition, RuntimeFlowDestination, RuntimeFlowEntry, RuntimeFlowNodeId,
    SemanticGameId, SemanticLevelId,
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
        ArtifactKind, DefinitionError, DefinitionErrorCode, DefinitionFireballBinding,
        DefinitionFlightBinding, DefinitionGoatPolicyBinding, ENEMY_DEFAULT_POLICY_ID,
        EXIT_SEQUENCE_OVERLAP_EXTENT_CAPABILITY_ID, EXPERIENCE_SCROLL_AWARD_CAPABILITY_ID,
        FIREBALL_MANA_COST_CAPABILITY_ID, GameDefinition, HOUSE_LIFE_MANA_SCALAR_CAPABILITY_ID,
        RequiredCapability, SemanticCatalog, SemanticId, creature_max_life_capability_id,
        default_catalog, engine_capabilities,
    },
    game_session::{
        CampaignSnapshot, EventPage, GameAdapter, GameAssetReference, GameAudioCommand,
        GameCommand, GameCompletion, GameCompletionStatus, GameEvent, GameSessionBackend,
        LateralControl, LongitudinalControl, PlayerMotion, PlayerPose, PlayerVitals,
        PresentationEntity, PresentationPose, RuntimeActorId, RuntimeParticipantId,
    },
    level1_mission::FIREBALL_LOGICAL_HEIGHT,
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
    let mut artifacts = vec![artifact_node(
        "campaign.owner_export",
        ArtifactKind::World,
        "campaign/owner-export",
        &export_sha256,
        package_bytes - envelope_bytes,
        &[],
    )?];
    let mut artifact_payloads = BTreeMap::new();
    let mut package_dependencies = vec!["campaign.owner_export"];
    let mut bindings = Vec::new();
    for asset in descriptor.presentation_assets() {
        let payload = descriptor
            .read_member(&asset.member.member)
            .map_err(|error| {
                definition_error(
                    DefinitionErrorCode::HashMismatch,
                    &asset.member.member,
                    format!("cannot retain validated presentation payload: {error}"),
                )
            })?;
        artifacts.push(artifact_node(
            &asset.artifact_id,
            ArtifactKind::Asset,
            &format!("campaign/presentation/{}", asset.member.member),
            &asset.member.sha256,
            payload.len() as u64,
            &[],
        )?);
        package_dependencies.push(&asset.artifact_id);
        artifact_payloads.insert(asset.artifact_id.clone(), payload);
    }
    artifacts.push(artifact_node(
        "campaign.package",
        ArtifactKind::Definition,
        "campaign/package",
        &descriptor.package_sha256,
        package_bytes,
        &package_dependencies,
    )?);
    if let Some(base_asset) = descriptor.presentation_assets().first() {
        bindings.push((
            SemanticId::parse(&base_asset.slot_id)?,
            sim_definition::SemanticValue::Reference(format!(
                "sha256:{}",
                base_asset.member.sha256
            )),
        ));
    }
    bindings.push((
        SemanticId::parse(ENEMY_DEFAULT_POLICY_ID)?,
        sim_definition::SemanticValue::Enum("classic_goat_v1".to_owned()),
    ));
    let mut requirement_ids = vec![
        crate::game_definition::CREATURE_POLICY_CAPABILITY_ID,
        "engine.flight.fixed_point",
        "engine.spell.projectile",
    ];
    if descriptor
        .level_definition()
        .objectives()
        .iter()
        .any(|objective| {
            matches!(
                objective.condition(),
                sim_definition::level_definition::ObjectiveCondition::DefeatRivalWizard(_)
            )
        })
    {
        requirement_ids.push("engine.objective.defeat_rival_wizard");
    }
    if descriptor
        .level_definition()
        .objectives()
        .iter()
        .any(|objective| {
            matches!(
                objective.condition(),
                sim_definition::level_definition::ObjectiveCondition::CastleManaPercent(_)
            )
        })
    {
        requirement_ids.push("engine.objective.castle_mana_percent");
    }
    if descriptor
        .level_definition()
        .objectives()
        .iter()
        .any(|objective| {
            matches!(
                objective.condition(),
                sim_definition::level_definition::ObjectiveCondition::NoLivingCreatures(_)
            )
        })
    {
        requirement_ids.push("engine.objective.no_living_creatures");
    }
    if descriptor
        .level_definition()
        .objectives()
        .iter()
        .any(|objective| {
            matches!(
                objective.condition(),
                sim_definition::level_definition::ObjectiveCondition::NoStandingBuildings
            )
        })
    {
        requirement_ids.push("engine.objective.no_standing_buildings");
    }
    if descriptor.local_player_fireball_mana_cost().is_some() {
        requirement_ids.push(FIREBALL_MANA_COST_CAPABILITY_ID);
    }
    if descriptor
        .level_definition()
        .entities()
        .iter()
        .any(|entity| entity.experience_scroll().is_some())
    {
        requirement_ids.push(EXPERIENCE_SCROLL_AWARD_CAPABILITY_ID);
    }
    if descriptor
        .level_definition()
        .entities()
        .iter()
        .any(|entity| entity.house_construction().is_some())
    {
        requirement_ids.push(HOUSE_LIFE_MANA_SCALAR_CAPABILITY_ID);
    }
    if descriptor
        .level_definition()
        .entities()
        .iter()
        .any(|entity| entity.switch_overlap().is_some())
    {
        requirement_ids.push(EXIT_SEQUENCE_OVERLAP_EXTENT_CAPABILITY_ID);
    }
    for entity in descriptor
        .level_definition()
        .entities()
        .iter()
        .filter(|entity| entity.creature_health().is_some())
    {
        let sim_definition::level_definition::LevelEntityKind::Creature(kind) = entity.kind()
        else {
            continue;
        };
        if let Some(capability_id) = creature_max_life_capability_id(kind)
            && !requirement_ids.contains(&capability_id)
        {
            requirement_ids.push(capability_id);
        }
    }
    if !descriptor.presentation_assets().is_empty() {
        requirement_ids.push("engine.presentation.indexed_asset");
    }
    let requirements = requirement_ids
        .into_iter()
        .map(|id| {
            Ok(RequiredCapability {
                id: SemanticId::parse(id)?,
                exact_version: 1,
            })
        })
        .collect::<crate::game_definition::Result<Vec<_>>>()?;
    let package = game_package_with_payloads(
        "mc2.campaign",
        1,
        requirements,
        artifacts,
        bindings,
        owner_provenance(&descriptor.package_sha256)?,
        artifact_payloads,
    )?;
    let runtime_flow = mc2_runtime_flow(selection, descriptor.level_definition())?;
    GameDefinition::from_package(package, &engine_capabilities()?, catalog).map(|definition| {
        with_level_and_runtime_flow(
            definition,
            descriptor.level_definition().clone(),
            runtime_flow,
        )
    })
}

fn mc2_runtime_flow(
    selection: CampaignSelection,
    level: &sim_definition::level_definition::LevelDefinition,
) -> crate::game_definition::Result<RuntimeFlowDefinition> {
    let (bound_level, witnesses): (&str, &[&str]) = match selection {
        CampaignSelection::LEVEL1 => (
            "level.magic_carpet_2.night_spire",
            &[
                "objective.reach.foot_of_spire",
                "objective.clear.spire_archers",
            ],
        ),
        CampaignSelection::LEVEL2 => (
            "level.magic_carpet_2.payahandra_ruins",
            &[
                "objective.defeat.nyphur",
                "objective.reach.payahandra_ruins",
            ],
        ),
        CampaignSelection::LEVEL3 => (
            "level.magic_carpet_2.revealed_way",
            &[
                "objective.clear.guardian_dragons",
                "objective.charge.reveal_entrance",
            ],
        ),
        _ => {
            return Err(definition_error(
                DefinitionErrorCode::InvalidSchema,
                "mc2_campaign.runtime_flow",
                "campaign selection has no qualified semantic runtime-flow binding",
            ));
        }
    };
    for witness in witnesses {
        let objective = sim_definition::adapter::stable_objective_id(witness)?;
        if level.objective(&objective).is_none() {
            return Err(definition_error(
                DefinitionErrorCode::InvalidSchema,
                "mc2_campaign.runtime_flow",
                format!("semantic level binding is missing package witness {witness}"),
            ));
        }
    }

    let spire = SemanticLevelId::parse("level.magic_carpet_2.night_spire")?;
    let payahandra = SemanticLevelId::parse("level.magic_carpet_2.payahandra_ruins")?;
    let entrance = SemanticLevelId::parse("level.magic_carpet_2.revealed_way")?;
    RuntimeFlowDefinition::new(
        SemanticGameId::parse("game.magic_carpet_2")?,
        SemanticLevelId::parse(bound_level)?,
        spire.clone(),
        vec![
            RuntimeFlowEntry::new(
                spire,
                RuntimeFlowDestination::Level {
                    level_id: payahandra.clone(),
                },
            ),
            RuntimeFlowEntry::new(
                payahandra,
                RuntimeFlowDestination::Level {
                    level_id: entrance.clone(),
                },
            ),
            RuntimeFlowEntry::new(
                entrance,
                RuntimeFlowDestination::Title {
                    node_id: RuntimeFlowNodeId::parse("flow.magic_carpet_2.return_to_title")?,
                },
            ),
        ],
    )
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

pub const MC2_CAMPAIGN_REPLAY_SAVE_CONTRACT: &str = "carpet-flight-mc2-campaign-replay-save/v2";
pub const MAX_MC2_CAMPAIGN_REPLAY_SAVE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_MC2_CAMPAIGN_REPLAY_COMMANDS: usize = 1_000_000;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Mc2CampaignReplaySave {
    save_contract: String,
    paused: bool,
    definition: GameDefinition,
    commands: Vec<GameCommand>,
    expected_tick: u64,
    expected_state_hash: u64,
    expected_terrain_hash: u64,
    expected_effect_head: u64,
    expected_effect_journal_sha256: String,
    receipt_sha256: String,
}

#[derive(Serialize)]
struct Mc2CampaignReplaySavePayload<'a> {
    save_contract: &'a str,
    paused: bool,
    definition: &'a GameDefinition,
    commands: &'a [GameCommand],
    expected_tick: u64,
    expected_state_hash: u64,
    expected_terrain_hash: u64,
    expected_effect_head: u64,
    expected_effect_journal_sha256: &'a str,
}

impl Mc2CampaignReplaySave {
    fn new(
        paused: bool,
        definition: GameDefinition,
        commands: Vec<GameCommand>,
        snapshot: &CampaignSnapshot,
    ) -> Self {
        let mut save = Self {
            save_contract: MC2_CAMPAIGN_REPLAY_SAVE_CONTRACT.to_owned(),
            paused,
            definition,
            commands,
            expected_tick: snapshot.tick,
            expected_state_hash: snapshot.state_hash,
            expected_terrain_hash: snapshot.terrain_hash,
            expected_effect_head: snapshot.effect_head,
            expected_effect_journal_sha256: snapshot.effect_journal_sha256.clone(),
            receipt_sha256: String::new(),
        };
        save.receipt_sha256 = save.compute_receipt_sha256();
        save
    }

    fn compute_receipt_sha256(&self) -> String {
        let payload = Mc2CampaignReplaySavePayload {
            save_contract: &self.save_contract,
            paused: self.paused,
            definition: &self.definition,
            commands: &self.commands,
            expected_tick: self.expected_tick,
            expected_state_hash: self.expected_state_hash,
            expected_terrain_hash: self.expected_terrain_hash,
            expected_effect_head: self.expected_effect_head,
            expected_effect_journal_sha256: &self.expected_effect_journal_sha256,
        };
        let bytes =
            serde_json::to_vec(&payload).expect("MC2 replay-save receipt payload is serializable");
        hex_digest(Sha256::digest(bytes).into())
    }

    fn verify(&self) -> bool {
        self.save_contract == MC2_CAMPAIGN_REPLAY_SAVE_CONTRACT
            && self.receipt_sha256 == self.compute_receipt_sha256()
    }
}

#[derive(Debug)]
pub enum Mc2CampaignReplaySaveError {
    InvalidMode,
    InvalidSave { message: String },
    Definition(DefinitionError),
    Runtime(crate::game_runtime::RuntimeTransitionError),
    Restore(crate::game_runtime::RestorePlayingError<CampaignBackendError>),
}

impl fmt::Display for Mc2CampaignReplaySaveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMode => formatter.write_str(
                "MC2 campaign replay saves require a Playing or quiescent Paused runtime",
            ),
            Self::InvalidSave { message } => {
                write!(formatter, "invalid MC2 replay save: {message}")
            }
            Self::Definition(error) => error.fmt(formatter),
            Self::Runtime(error) => error.fmt(formatter),
            Self::Restore(error) => error.fmt(formatter),
        }
    }
}

impl Error for Mc2CampaignReplaySaveError {}

/// One definition-validated MC2 resource binding, keyed only by the semantic
/// level identity derived from its immutable definition. Owner-cache and
/// backend materialization validation remain retryable work at open time.
#[derive(Clone, Debug)]
struct ResolvedMc2RuntimePackage {
    definition: Arc<GameDefinition>,
    inputs: Mc2CampaignInputs,
}

/// Immutable installed-package resolver for one closed semantic MC2 flow.
///
/// Callers supply only unordered resource locations. The resolver loads and
/// verifies every definition, derives each key from `bound_level_id`, and
/// rejects mixed, duplicate, or incomplete package sets before a runtime can
/// enter Loading. It never accepts a caller-provided next-level identity.
#[derive(Clone, Debug)]
pub struct Mc2RuntimePackageResolver {
    flow: RuntimeFlowDefinition,
    by_level: BTreeMap<SemanticLevelId, ResolvedMc2RuntimePackage>,
}

impl Mc2RuntimePackageResolver {
    fn exact_package_for_definition(
        &self,
        definition: &GameDefinition,
    ) -> Result<&ResolvedMc2RuntimePackage, crate::game_runtime::RuntimeTransitionError> {
        let level_id = definition
            .runtime_flow()
            .map(RuntimeFlowDefinition::bound_level_id)
            .ok_or_else(
                || crate::game_runtime::RuntimeTransitionError::InvalidRuntimeFlow {
                    message: "resolver-bound definition has no semantic runtime flow".to_owned(),
                },
            )?;
        let package = self.package(level_id).ok_or_else(|| {
            crate::game_runtime::RuntimeTransitionError::InvalidRuntimeFlow {
                message: format!(
                    "installed package resolver has no binding for {}",
                    level_id.as_str()
                ),
            }
        })?;
        if definition.package_graph_hash() != package.definition.package_graph_hash()
            || definition.definition_hash() != package.definition.definition_hash()
            || definition.revision() != package.definition.revision()
        {
            return Err(
                crate::game_runtime::RuntimeTransitionError::InvalidRuntimeFlow {
                    message: format!(
                        "definition {} is not the resolver's exact installed binding",
                        level_id.as_str()
                    ),
                },
            );
        }
        Ok(package)
    }

    /// Replace one installed definition only for a resolver-binding negative.
    /// The returned exact binding must be restored before the positive route.
    #[cfg(test)]
    pub(crate) fn replace_exact_definition_for_test(
        &mut self,
        definition: Arc<GameDefinition>,
    ) -> Result<Arc<GameDefinition>, crate::game_runtime::RuntimeTransitionError> {
        let level_id = definition
            .runtime_flow()
            .map(|flow| flow.bound_level_id().clone())
            .ok_or_else(
                || crate::game_runtime::RuntimeTransitionError::InvalidRuntimeFlow {
                    message: "test replacement definition has no semantic runtime flow".to_owned(),
                },
            )?;
        let package = self.by_level.get_mut(&level_id).ok_or_else(|| {
            crate::game_runtime::RuntimeTransitionError::InvalidRuntimeFlow {
                message: format!(
                    "installed package resolver has no binding for {}",
                    level_id.as_str()
                ),
            }
        })?;
        Ok(std::mem::replace(&mut package.definition, definition))
    }

    /// Load and close one unordered installed package set over its declared
    /// semantic runtime flow.
    ///
    /// # Errors
    ///
    /// Returns a typed definition error when a package is invalid, a derived
    /// level identity is duplicated, flow identities disagree, or any
    /// declared playable level is absent.
    pub fn load(
        inputs: impl IntoIterator<Item = Mc2CampaignInputs>,
    ) -> crate::game_definition::Result<Self> {
        let catalog = default_catalog()?;
        let mut flow: Option<RuntimeFlowDefinition> = None;
        let mut by_level = BTreeMap::new();
        for input in inputs {
            let definition = Arc::new(load_mc2_campaign(&input.package_root, &catalog)?);
            definition.verify_identity()?;
            let candidate = definition.runtime_flow().cloned().ok_or_else(|| {
                definition_error(
                    DefinitionErrorCode::InvalidSchema,
                    "runtime_package_resolver.flow",
                    "installed campaign definition has no semantic runtime flow",
                )
            })?;
            if let Some(expected) = &flow {
                if candidate.game_id() != expected.game_id()
                    || candidate.entry_level_id() != expected.entry_level_id()
                    || candidate.entries() != expected.entries()
                {
                    return Err(definition_error(
                        DefinitionErrorCode::DependencyFailed,
                        "runtime_package_resolver.flow",
                        "installed campaign definitions do not share one ordered semantic flow",
                    ));
                }
            } else {
                flow = Some(candidate.clone());
            }
            let level_id = candidate.bound_level_id().clone();
            if by_level
                .insert(
                    level_id.clone(),
                    ResolvedMc2RuntimePackage {
                        definition,
                        inputs: input,
                    },
                )
                .is_some()
            {
                return Err(definition_error(
                    DefinitionErrorCode::DuplicateBinding,
                    level_id.as_str(),
                    "installed campaign package set duplicates a semantic level binding",
                ));
            }
        }
        let observed_flow = flow.ok_or_else(|| {
            definition_error(
                DefinitionErrorCode::InvalidSchema,
                "runtime_package_resolver.packages",
                "installed campaign package set is empty",
            )
        })?;
        for entry in observed_flow.entries() {
            if !by_level.contains_key(entry.level_id()) {
                return Err(definition_error(
                    DefinitionErrorCode::DependencyFailed,
                    entry.level_id().as_str(),
                    "installed campaign package set is missing a declared playable level",
                ));
            }
        }
        if by_level.len() != observed_flow.entries().len() {
            return Err(definition_error(
                DefinitionErrorCode::InvalidSchema,
                "runtime_package_resolver.packages",
                "installed campaign package set contains a level outside its closed flow",
            ));
        }
        let flow = by_level
            .get(observed_flow.entry_level_id())
            .and_then(|package| package.definition.runtime_flow())
            .cloned()
            .expect("closed package set contains its validated entry definition");
        Ok(Self { flow, by_level })
    }

    #[must_use]
    pub const fn flow(&self) -> &RuntimeFlowDefinition {
        &self.flow
    }

    fn package(&self, level_id: &SemanticLevelId) -> Option<&ResolvedMc2RuntimePackage> {
        self.by_level.get(level_id)
    }

    /// Bind the resolver-derived entry package at the runtime's guarded
    /// new-game boundary.
    pub fn new_game(
        &self,
        runtime: &mut crate::game_runtime::GameRuntime<Mc2CampaignBackend>,
        guard: crate::game_runtime::RuntimeGuard,
    ) -> Result<crate::game_runtime::RuntimeIdentity, crate::game_runtime::RuntimeTransitionError>
    {
        let package = self
            .package(self.flow.entry_level_id())
            .expect("closed installed package set contains its entry level");
        runtime.new_game(guard, Arc::clone(&package.definition))
    }

    /// Open the currently pending resolver-owned package through the ordinary
    /// MC2 gameplay adapter.
    pub fn open_pending(
        &self,
        runtime: &mut crate::game_runtime::GameRuntime<Mc2CampaignBackend>,
        guard: crate::game_runtime::RuntimeGuard,
        session_id: u64,
    ) -> Result<
        crate::game_runtime::RuntimeIdentity,
        crate::game_runtime::OpenLevelError<CampaignBackendError>,
    > {
        runtime.open_level_resolved(guard, &Mc2CampaignAdapter, session_id, |definition| {
            self.exact_package_for_definition(definition)
                .map(|package| package.inputs.clone())
        })
    }

    /// Serialize one Playing or quiescent Paused runtime as a definition-bound
    /// semantic command replay plus exact authoritative-state receipts.
    pub fn save_replay(
        &self,
        runtime: &crate::game_runtime::GameRuntime<Mc2CampaignBackend>,
    ) -> Result<Vec<u8>, Mc2CampaignReplaySaveError> {
        let mode = runtime.identity().mode;
        if !matches!(
            mode,
            crate::game_runtime::RuntimeMode::Playing | crate::game_runtime::RuntimeMode::Paused
        ) {
            return Err(Mc2CampaignReplaySaveError::InvalidMode);
        }
        let session = runtime
            .session()
            .ok_or(Mc2CampaignReplaySaveError::InvalidMode)?;
        let flow = session.definition().runtime_flow().ok_or_else(|| {
            Mc2CampaignReplaySaveError::InvalidSave {
                message: "active definition has no semantic runtime flow".to_owned(),
            }
        })?;
        let package = self.package(flow.bound_level_id()).ok_or_else(|| {
            Mc2CampaignReplaySaveError::InvalidSave {
                message: format!(
                    "installed resolver has no package for {}",
                    flow.bound_level_id().as_str()
                ),
            }
        })?;
        let catalog = default_catalog().map_err(Mc2CampaignReplaySaveError::Definition)?;
        session
            .definition()
            .verify_semantic_variant_of(&package.definition, &catalog)
            .map_err(Mc2CampaignReplaySaveError::Definition)?;
        let snapshot = session.backend().snapshot();
        if session.backend().replay_commands.len() > MAX_MC2_CAMPAIGN_REPLAY_COMMANDS {
            return Err(Mc2CampaignReplaySaveError::InvalidSave {
                message: format!(
                    "session has {} commands, exceeding the {MAX_MC2_CAMPAIGN_REPLAY_COMMANDS}-command bound",
                    session.backend().replay_commands.len()
                ),
            });
        }
        let save = Mc2CampaignReplaySave::new(
            mode == crate::game_runtime::RuntimeMode::Paused,
            session.definition().as_ref().clone(),
            session.backend().replay_commands.clone(),
            &snapshot,
        );
        let bytes =
            serde_json::to_vec(&save).map_err(|error| Mc2CampaignReplaySaveError::InvalidSave {
                message: format!("cannot serialize replay save: {error}"),
            })?;
        if bytes.len() > MAX_MC2_CAMPAIGN_REPLAY_SAVE_BYTES {
            return Err(Mc2CampaignReplaySaveError::InvalidSave {
                message: format!(
                    "save has {} bytes, exceeding the {MAX_MC2_CAMPAIGN_REPLAY_SAVE_BYTES}-byte bound",
                    bytes.len()
                ),
            });
        }
        Ok(bytes)
    }

    /// Restore one replay save directly from Title into Playing or quiescent
    /// Paused. The candidate is published only when every retained receipt
    /// matches exactly.
    pub fn load_replay(
        &self,
        runtime: &mut crate::game_runtime::GameRuntime<Mc2CampaignBackend>,
        guard: crate::game_runtime::RuntimeGuard,
        session_id: u64,
        bytes: &[u8],
    ) -> Result<crate::game_runtime::RuntimeIdentity, Mc2CampaignReplaySaveError> {
        runtime
            .preflight_title_transition(guard)
            .map_err(Mc2CampaignReplaySaveError::Runtime)?;
        if bytes.len() > MAX_MC2_CAMPAIGN_REPLAY_SAVE_BYTES {
            return Err(Mc2CampaignReplaySaveError::InvalidSave {
                message: format!(
                    "save has {} bytes, exceeding the {MAX_MC2_CAMPAIGN_REPLAY_SAVE_BYTES}-byte bound",
                    bytes.len()
                ),
            });
        }
        let save: Mc2CampaignReplaySave = serde_json::from_slice(bytes).map_err(|error| {
            Mc2CampaignReplaySaveError::InvalidSave {
                message: format!("save JSON is invalid: {error}"),
            }
        })?;
        if !save.verify() {
            return Err(Mc2CampaignReplaySaveError::InvalidSave {
                message: "save contract or receipt digest does not verify".to_owned(),
            });
        }
        if save.commands.len() > MAX_MC2_CAMPAIGN_REPLAY_COMMANDS {
            return Err(Mc2CampaignReplaySaveError::InvalidSave {
                message: format!(
                    "save has {} commands, exceeding the {MAX_MC2_CAMPAIGN_REPLAY_COMMANDS}-command bound",
                    save.commands.len()
                ),
            });
        }
        save.definition
            .verify_identity()
            .map_err(Mc2CampaignReplaySaveError::Definition)?;
        let flow = save.definition.runtime_flow().ok_or_else(|| {
            Mc2CampaignReplaySaveError::InvalidSave {
                message: "saved definition has no semantic runtime flow".to_owned(),
            }
        })?;
        let package = self.package(flow.bound_level_id()).ok_or_else(|| {
            Mc2CampaignReplaySaveError::InvalidSave {
                message: format!(
                    "installed resolver has no package for {}",
                    flow.bound_level_id().as_str()
                ),
            }
        })?;
        let catalog = default_catalog().map_err(Mc2CampaignReplaySaveError::Definition)?;
        save.definition
            .verify_semantic_variant_of(&package.definition, &catalog)
            .map_err(Mc2CampaignReplaySaveError::Definition)?;
        let definition = Arc::new(save.definition);
        let commands = save.commands;
        let expected_tick = save.expected_tick;
        let expected_state_hash = save.expected_state_hash;
        let expected_terrain_hash = save.expected_terrain_hash;
        let expected_effect_head = save.expected_effect_head;
        let expected_effect_journal_sha256 = save.expected_effect_journal_sha256;
        let paused = save.paused;
        runtime
            .restore_playing_resolved(
                guard,
                definition,
                &Mc2CampaignAdapter,
                session_id,
                commands,
                paused,
                |_| Ok(package.inputs.clone()),
                move |candidate| {
                    let actual = candidate.backend().snapshot();
                    if actual.tick != expected_tick
                        || actual.state_hash != expected_state_hash
                        || actual.terrain_hash != expected_terrain_hash
                        || actual.effect_head != expected_effect_head
                        || actual.effect_journal_sha256 != expected_effect_journal_sha256
                    {
                        return Err(crate::game_runtime::RuntimeTransitionError::InvalidReplay {
                            message: "replayed campaign state does not match the saved receipts"
                                .to_owned(),
                        });
                    }
                    Ok(())
                },
            )
            .map_err(Mc2CampaignReplaySaveError::Restore)
    }

    /// Reseal a save with one deliberately wrong expected state for the
    /// replay-verification negative. Gameplay commands and definitions remain
    /// untouched, so only a loader that actually replays can reject it.
    #[cfg(test)]
    pub(crate) fn corrupt_save_expected_state_for_test(bytes: &[u8]) -> Vec<u8> {
        let mut save: Mc2CampaignReplaySave = serde_json::from_slice(bytes).unwrap();
        save.expected_state_hash ^= 1;
        save.receipt_sha256 = save.compute_receipt_sha256();
        serde_json::to_vec(&save).unwrap()
    }

    /// Resolve the active semantic completion edge and accept it without a
    /// caller-provided level identity, definition, or package path.
    pub fn accept_transition(
        &self,
        runtime: &mut crate::game_runtime::GameRuntime<Mc2CampaignBackend>,
        guard: crate::game_runtime::RuntimeGuard,
        session_guard: crate::game_session::SessionGuard,
    ) -> Result<
        crate::game_runtime::CompletionTransitionReceipt,
        crate::game_runtime::RuntimeTransitionError,
    > {
        runtime.accept_transition_resolved(
            guard,
            session_guard,
            |source_definition, destination| {
                self.exact_package_for_definition(source_definition)?;
                match destination {
                    RuntimeFlowDestination::Level { level_id } => self
                        .package(level_id)
                        .map(|package| {
                            crate::game_runtime::ResolvedCompletionTarget::Level(Arc::clone(
                                &package.definition,
                            ))
                        })
                        .ok_or_else(|| {
                            crate::game_runtime::RuntimeTransitionError::InvalidRuntimeFlow {
                                message: format!(
                                    "installed package resolver has no binding for {}",
                                    level_id.as_str()
                                ),
                            }
                        }),
                    RuntimeFlowDestination::Title { .. } => {
                        Ok(crate::game_runtime::ResolvedCompletionTarget::Title)
                    }
                }
            },
        )
    }
}

/// Definition-bound campaign backend. The wrapped [`CampaignSession`] owns
/// live gameplay state, ordering, RNG, journals, reset, and replay.
#[derive(Debug)]
pub struct Mc2CampaignBackend {
    campaign: CampaignSession,
    package_campaign: CampaignSession,
    flight_binding: DefinitionFlightBinding,
    fireball_binding: Option<DefinitionFireballBinding>,
    fireball_projectile_asset: Option<GameAssetReference>,
    goat_policy_binding: Option<DefinitionGoatPolicyBinding>,
    package_base: GameDefinition,
    package_graph_hash: String,
    artifact_graph_hash: String,
    replay_commands: Vec<GameCommand>,
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
        let fireball_binding = DefinitionFireballBinding::from_definition(definition)
            .map_err(CampaignBackendError::Definition)?;
        let goat_policy_binding = DefinitionGoatPolicyBinding::from_definition(definition)
            .map_err(CampaignBackendError::Definition)?;
        let fireball_projectile_asset = fireball_projectile_asset_reference(definition)?;
        require_catalog_defaults(
            definition,
            &catalog,
            fireball_binding.is_some(),
            goat_policy_binding.is_some(),
            consumes_fireball_projectile_asset(definition),
        )?;
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
        if let Some(binding) = goat_policy_binding {
            campaign.apply_enemy_goat_policy(binding);
        }
        if let Some(binding) = fireball_binding {
            campaign
                .apply_local_player_fireball_mana_cost(binding.mana_cost())
                .map_err(|error| {
                    CampaignBackendError::Open(CampaignOpenError::Materialization(error))
                })?;
        }
        Ok(Mc2CampaignBackend {
            campaign,
            package_campaign,
            flight_binding,
            fireball_binding,
            fireball_projectile_asset,
            goat_policy_binding,
            package_graph_hash: definition.package_graph_hash().to_owned(),
            artifact_graph_hash: definition.artifact_graph_hash().to_owned(),
            replay_commands: Vec::new(),
            package_base,
        })
    }
}

/// Admit only semantic deviations with a package-present runtime consumer.
/// Every other catalog value must remain at its package default so this route
/// cannot silently bind a definition whose authored value gameplay ignores.
fn require_catalog_defaults(
    definition: &GameDefinition,
    catalog: &SemanticCatalog,
    consumes_fireball_cost: bool,
    consumes_goat_policy: bool,
    consumes_fireball_projectile_asset: bool,
) -> Result<(), CampaignBackendError> {
    for descriptor in catalog.descriptors() {
        if DefinitionFlightBinding::consumes(&descriptor.id)
            || (consumes_fireball_cost && DefinitionFireballBinding::consumes(&descriptor.id))
            || (consumes_goat_policy && DefinitionGoatPolicyBinding::consumes(&descriptor.id))
            || (consumes_fireball_projectile_asset
                && descriptor.id.as_str() == "presentation.spell.fireball.projectile_asset")
        {
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

fn consumes_fireball_projectile_asset(definition: &GameDefinition) -> bool {
    let Ok(id) = SemanticId::parse("presentation.spell.fireball.projectile_asset") else {
        return false;
    };
    definition
        .required_capabilities()
        .iter()
        .any(|requirement| {
            requirement.id.as_str() == "engine.presentation.indexed_asset"
                && requirement.exact_version == 1
        })
        && definition.presentation_asset(&id).is_ok()
}

fn fireball_projectile_asset_reference(
    definition: &GameDefinition,
) -> Result<Option<GameAssetReference>, CampaignBackendError> {
    if !consumes_fireball_projectile_asset(definition) {
        return Ok(None);
    }
    let id = SemanticId::parse("presentation.spell.fireball.projectile_asset")
        .map_err(CampaignBackendError::Definition)?;
    let asset = definition
        .presentation_asset(&id)
        .map_err(CampaignBackendError::Definition)?;
    Ok(Some(GameAssetReference {
        id: id.as_str().to_owned(),
        content_address: asset.content_address,
    }))
}

impl GameSessionBackend for Mc2CampaignBackend {
    type Command = GameCommand;
    type Snapshot = CampaignSnapshot;
    type Error = CampaignBackendError;

    fn tick(&self) -> u64 {
        self.campaign.tick()
    }

    fn completion_status(&self) -> GameCompletionStatus {
        let exit = self.campaign.level2_compatibility().level_exit_sequence();
        if !exit.complete {
            GameCompletionStatus::InProgress
        } else if exit.result_code == 10 {
            GameCompletionStatus::Completed {
                completion: GameCompletion {
                    state_hash: self.campaign.state_hash(),
                },
            }
        } else {
            GameCompletionStatus::Invalid {
                message: "completed MC2 exit carrier has a non-success result".to_owned(),
            }
        }
    }

    fn snapshot(&self) -> Self::Snapshot {
        let flight = &self.campaign.level2.state;
        let shared_spells = self.campaign.level2_compatibility().shared_spells();
        let player_entity = shared_spells.player_entity();
        let presentation_entities = self
            .fireball_projectile_asset
            .as_ref()
            .map(|asset| {
                let kind = SemanticId::parse("spell.fireball.projectile")
                    .expect("compiled semantic Fireball projectile ID");
                self.campaign
                    .level2_compatibility()
                    .economy()
                    .fireball_projectiles()
                    .iter()
                    .filter(|projectile| projectile.active && !projectile.drawing_disabled)
                    .map(|projectile| PresentationEntity {
                        actor: self
                            .actor(u64::from(projectile.source_entity_index))
                            .expect("opaque actor projection is total"),
                        kind: kind.clone(),
                        pose: PresentationPose {
                            x: projectile.position.x,
                            y: projectile.position.y,
                            z: projectile.position.z,
                            yaw: projectile.yaw,
                            pitch: projectile.pitch,
                        },
                        logical_height: FIREBALL_LOGICAL_HEIGHT,
                        asset: asset.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
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
            player_vitals: PlayerVitals {
                life: player_entity.life,
                max_life: player_entity.max_life,
                mana: shared_spells.player_mana(),
                max_mana: player_entity.max_mana,
            },
            presentation_entities,
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
        self.replay_commands.push(command);
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
        let fireball_binding = DefinitionFireballBinding::from_definition(definition)
            .map_err(CampaignBackendError::Definition)?;
        let goat_policy_binding = DefinitionGoatPolicyBinding::from_definition(definition)
            .map_err(CampaignBackendError::Definition)?;
        let fireball_projectile_asset = fireball_projectile_asset_reference(definition)?;
        definition
            .verify_semantic_variant_of(&self.package_base, &catalog)
            .map_err(CampaignBackendError::Definition)?;
        require_catalog_defaults(
            definition,
            &catalog,
            fireball_binding.is_some(),
            goat_policy_binding.is_some(),
            consumes_fireball_projectile_asset(definition),
        )?;
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
        if let Some(binding) = goat_policy_binding {
            campaign.apply_enemy_goat_policy(binding);
        }
        if let Some(binding) = fireball_binding {
            campaign
                .apply_local_player_fireball_mana_cost(binding.mana_cost())
                .map_err(|error| {
                    CampaignBackendError::Open(CampaignOpenError::Materialization(error))
                })?;
        }
        campaign.set_runtime_generation(next_generation);
        self.campaign = campaign;
        self.flight_binding = flight_binding;
        self.fireball_binding = fireball_binding;
        self.fireball_projectile_asset = fireball_projectile_asset;
        self.goat_policy_binding = goat_policy_binding;
        self.replay_commands.clear();
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

    #[cfg(test)]
    pub(crate) fn compatibility_campaign_mut_for_test(&mut self) -> &mut CampaignSession {
        &mut self.campaign
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
    use crate::campaign::state::{ResetReceipt, ResetState, encode_reset_v1};
    use crate::game_definition::{
        ARCHER_MAX_LIFE_CAPABILITY_ID, Access, ArtifactId, BEE_MAX_LIFE_CAPABILITY_ID,
        ChangeSetBuilder, DRAGON_MAX_LIFE_CAPABILITY_ID, FIREFLY_MAX_LIFE_CAPABILITY_ID,
        GOAT_MAX_LIFE_CAPABILITY_ID, GamePackage, IntegerParameter, OptionParameter,
        PresentationAssetSlot, PublicationFault, RequestedTiming, SKELETON_MAX_LIFE_CAPABILITY_ID,
        SemanticValue, TRADER_MAX_LIFE_CAPABILITY_ID, VILLAGER_MAX_LIFE_CAPABILITY_ID,
        WORM_MAX_LIFE_CAPABILITY_ID, WorkspaceStore,
    };
    use crate::game_session::{GameSession, SessionGuard};
    use crate::goat::{GoatBehaviorConfig, GoatStepEvent};
    use crate::level_definition::{
        ActivationGroupId, BuildingKind, CreatureKind, ItemKind, LevelEntityKind,
        LevelEntityParameterAccess, LevelEntityPositionAccess, LevelObjectiveConditionAccess,
        MechanicKind, ObjectiveCondition, SceneryKind, SpellKind, SwitchKind, TilePosition,
        supports_authorable_creature_placement,
    };
    use crate::native_world::FireballProjectileOrigin;
    use sim_definition::adapter::{
        build_level_definition, entity_definition, game_package, objective_definition,
        reissue_at_revision, with_level_definition, with_replacement,
    };
    use std::{
        collections::BTreeSet,
        path::PathBuf,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn descriptor_fixture_path() -> PathBuf {
        std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2")
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
        std::env::var_os("MC_CAMPAIGN_LEVEL1_PACKAGE").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf15-level1-package-v3-current")
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

    #[test]
    fn runtime_package_resolver_canonicalizes_order_and_rejects_open_flow_sets() {
        let level_one = level_one_descriptor_fixture_path();
        let level_two = descriptor_fixture_path();
        let level_three = level_three_descriptor_fixture_path();
        let forward = Mc2RuntimePackageResolver::load([
            Mc2CampaignInputs::new(&level_one),
            Mc2CampaignInputs::new(&level_two),
            Mc2CampaignInputs::new(&level_three),
        ])
        .unwrap();
        let reverse = Mc2RuntimePackageResolver::load([
            Mc2CampaignInputs::new(&level_three),
            Mc2CampaignInputs::new(&level_two),
            Mc2CampaignInputs::new(&level_one),
        ])
        .unwrap();
        assert_eq!(forward.flow(), reverse.flow());
        assert_eq!(
            forward.flow().bound_level_id(),
            forward.flow().entry_level_id()
        );

        let incomplete = Mc2RuntimePackageResolver::load([
            Mc2CampaignInputs::new(&level_one),
            Mc2CampaignInputs::new(&level_three),
        ])
        .unwrap_err();
        assert_eq!(incomplete.code, DefinitionErrorCode::DependencyFailed);

        let duplicate = Mc2RuntimePackageResolver::load([
            Mc2CampaignInputs::new(&level_one),
            Mc2CampaignInputs::new(&level_one),
            Mc2CampaignInputs::new(&level_two),
            Mc2CampaignInputs::new(&level_three),
        ])
        .unwrap_err();
        assert_eq!(duplicate.code, DefinitionErrorCode::DuplicateBinding);

        // No-Claim: this constructor gate proves only definition-derived,
        // order-independent closure for the three qualified resource roots.
        // It does not validate owner-cache bytes, install/discover packages,
        // open gameplay, or consume a completion edge.
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

    fn publish_fireball_mana_cost(
        store: &WorkspaceStore,
        catalog: &SemanticCatalog,
        active: &GameDefinition,
        idempotency_key: &str,
        mana_cost: i64,
    ) -> GameDefinition {
        let change_set = ChangeSetBuilder::new(
            "campaign-fireball-authoring",
            active,
            idempotency_key,
            RequestedTiming::SessionReset,
        )
        .set_integer(IntegerParameter::FireballManaCost, mana_cost)
        .build()
        .expect("typed Fireball cost change set");
        let validation = store
            .validate(catalog, change_set.clone())
            .expect("Fireball cost change validates");
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .expect("Fireball cost change publishes");
        store.active_definition().expect("published definition")
    }

    fn publish_experience_scroll_award(
        store: &WorkspaceStore,
        catalog: &SemanticCatalog,
        active: &GameDefinition,
        entity: sim_definition::level_definition::LevelEntityId,
        award: u32,
    ) -> GameDefinition {
        let change_set = ChangeSetBuilder::new(
            "campaign-experience-scroll-award",
            active,
            "publish-experience-scroll-award",
            RequestedTiming::SessionReset,
        )
        .set_experience_scroll_award_from(entity, 4, award)
        .build()
        .expect("typed Experience Scroll change set");
        let validation = store
            .validate(catalog, change_set.clone())
            .expect("Experience Scroll change validates");
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .expect("Experience Scroll change publishes");
        store.active_definition().expect("published definition")
    }

    fn publish_enemy_goat_policy(
        store: &WorkspaceStore,
        catalog: &SemanticCatalog,
        active: &GameDefinition,
        idempotency_key: &str,
        option: &str,
    ) -> GameDefinition {
        let change_set = ChangeSetBuilder::new(
            "campaign-goat-policy-authoring",
            active,
            idempotency_key,
            RequestedTiming::SessionReset,
        )
        .select_option(OptionParameter::EnemyDefaultPolicy, option)
        .build()
        .expect("typed Goat policy change set");
        let validation = store
            .validate(catalog, change_set.clone())
            .expect("Goat policy change validates");
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .expect("Goat policy change publishes");
        store.active_definition().expect("published definition")
    }

    fn publish_fireball_projectile_asset(
        store: &WorkspaceStore,
        catalog: &SemanticCatalog,
        active: &GameDefinition,
        artifact_id: &str,
    ) -> GameDefinition {
        let change_set = ChangeSetBuilder::new(
            "campaign-fireball-presentation-authoring",
            active,
            "bind-fireball-projectile-presentation",
            RequestedTiming::PackageReload,
        )
        .bind_asset(
            PresentationAssetSlot::FireballProjectile,
            ArtifactId::parse(artifact_id).expect("stable retained asset ID"),
        )
        .build()
        .expect("typed presentation change set");
        let validation = store
            .validate(catalog, change_set.clone())
            .expect("presentation change validates");
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .expect("presentation change publishes");
        store.active_definition().expect("published definition")
    }

    fn assert_level_one_fireball_cost_mirrors(
        session: &GameSession<Mc2CampaignBackend>,
        expected: i32,
    ) {
        let compatibility = session.backend().campaign.level2_compatibility();
        let shared = compatibility.shared_spells();
        let holder_index = shared.players()[0].enabled_spell_holders[0];
        let holder = shared
            .holders()
            .iter()
            .find(|holder| holder.index == holder_index)
            .expect("local player Fireball holder remains typed");
        let imported = &compatibility.entities()[usize::from(holder_index)];
        let context = compatibility
            .session
            .state
            .live
            .core
            .context
            .spell_fire
            .as_ref()
            .expect("ordinary spell-fire context remains materialized");
        let context_holder = context
            .holders
            .iter()
            .find(|candidate| candidate.entity_index == holder_index)
            .expect("ordinary context retains the local Fireball holder");
        assert_eq!(
            [
                holder.max_mana,
                holder.mana_cost,
                i32::from_le_bytes(holder.record_bytes()[144..148].try_into().unwrap()),
                imported.max_mana,
                imported.mana_cost,
                i32::from_le_bytes(imported.record_bytes()[144..148].try_into().unwrap()),
                shared.fireball_holder().mana_cost,
                context_holder.mana_cost,
            ],
            [expected; 8]
        );
        assert_eq!(
            ResetReceipt::from_state(&compatibility.session.state.live),
            compatibility.session.baseline_hash,
            "the authored live carrier must be sealed as the reset baseline"
        );
    }

    fn player_spell_projectile_count(session: &GameSession<Mc2CampaignBackend>) -> usize {
        session
            .backend()
            .campaign
            .level2_compatibility()
            .economy()
            .fireball_projectiles()
            .iter()
            .filter(|projectile| projectile.origin == FireballProjectileOrigin::PlayerSpell)
            .count()
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

    fn publish_objective(
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
            "author-objective-condition",
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
        expected_reset_live_creatures: &[(CreatureKind, usize)],
        expected_spells: &[(SpellKind, TilePosition, Option<u16>)],
        expected_scenery: &[(SceneryKind, usize)],
        expected_items: &[(ItemKind, usize)],
        expected_buildings: &[(BuildingKind, usize)],
        expected_writable_houses: usize,
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
        for (kind, count) in expected_reset_live_creatures {
            assert_eq!(
                level
                    .entities()
                    .iter()
                    .filter(|entity| {
                        entity.kind() == LevelEntityKind::Creature(*kind)
                            && entity.activation_group().is_none()
                            && entity.position_access() == LevelEntityPositionAccess::SessionReset
                    })
                    .count(),
                *count
            );
        }
        for entity in level
            .entities()
            .iter()
            .filter(|entity| matches!(entity.kind(), LevelEntityKind::Creature(_)))
        {
            let LevelEntityKind::Creature(kind) = entity.kind() else {
                unreachable!("filtered to semantic creatures")
            };
            let reset_live = entity.activation_group().is_none()
                && expected_reset_live_creatures
                    .iter()
                    .any(|(expected, _)| *expected == kind);
            let expected_access = if supports_authorable_creature_placement(kind)
                && (entity.activation_group().is_some() || reset_live)
            {
                LevelEntityPositionAccess::SessionReset
            } else {
                LevelEntityPositionAccess::ReadOnly
            };
            assert_eq!(
                entity.position_access(),
                expected_access,
                "creature placement access diverged for {}",
                entity.id().as_str()
            );
            if kind == CreatureKind::Goat {
                let health = entity
                    .creature_health()
                    .expect("every package-present Goat has a health binding");
                assert_eq!(health.default_max_life(), 600);
                assert_eq!(health.max_life(), 600);
                assert_eq!(
                    health.max_life_access(),
                    LevelEntityParameterAccess::SessionReset
                );
            }
            if kind == CreatureKind::Trader {
                let health = entity
                    .creature_health()
                    .expect("every package-present Trader has a health binding");
                assert_eq!(health.default_max_life(), 1_000);
                assert_eq!(health.max_life(), 1_000);
                assert_eq!(
                    health.max_life_access(),
                    LevelEntityParameterAccess::SessionReset
                );
            }
            if kind == CreatureKind::Villager {
                let health = entity
                    .creature_health()
                    .expect("every package-present Villager has a health binding");
                assert_eq!(health.default_max_life(), 1_000);
                assert_eq!(health.max_life(), 1_000);
                assert_eq!(
                    health.max_life_access(),
                    LevelEntityParameterAccess::SessionReset
                );
            }
        }
        if expected_creatures
            .iter()
            .any(|(kind, _)| *kind == CreatureKind::Goat)
        {
            assert!(
                definition
                    .required_capabilities()
                    .iter()
                    .any(|requirement| {
                        requirement.id.as_str() == GOAT_MAX_LIFE_CAPABILITY_ID
                            && requirement.exact_version == 1
                    })
            );
        }
        if expected_creatures
            .iter()
            .any(|(kind, _)| *kind == CreatureKind::Trader)
        {
            assert!(
                definition
                    .required_capabilities()
                    .iter()
                    .any(|requirement| {
                        requirement.id.as_str() == TRADER_MAX_LIFE_CAPABILITY_ID
                            && requirement.exact_version == 1
                    })
            );
        }
        if expected_creatures
            .iter()
            .any(|(kind, _)| *kind == CreatureKind::Villager)
        {
            assert!(
                definition
                    .required_capabilities()
                    .iter()
                    .any(|requirement| {
                        requirement.id.as_str() == VILLAGER_MAX_LIFE_CAPABILITY_ID
                            && requirement.exact_version == 1
                    })
            );
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
        let experience_scrolls = level
            .entities()
            .iter()
            .filter(|entity| entity.kind() == LevelEntityKind::Item(ItemKind::ExperienceScroll))
            .collect::<Vec<_>>();
        assert_eq!(
            experience_scrolls.len(),
            expected_items
                .iter()
                .find(|(kind, _)| *kind == ItemKind::ExperienceScroll)
                .map_or(0, |(_, count)| *count)
        );
        assert!(experience_scrolls.iter().all(|entity| {
            entity.experience_scroll().is_some_and(|config| {
                config.default_experience_award() == 4
                    && config.experience_award() == 4
                    && config.experience_award_access() == LevelEntityParameterAccess::SessionReset
            })
        }));
        if !experience_scrolls.is_empty() {
            assert!(
                definition
                    .required_capabilities()
                    .iter()
                    .any(|requirement| {
                        requirement.id.as_str() == EXPERIENCE_SCROLL_AWARD_CAPABILITY_ID
                            && requirement.exact_version == 1
                    })
            );
        }
        assert_eq!(
            level
                .building_kinds()
                .map(|kind| (kind, level.authored_building_count(kind)))
                .collect::<Vec<_>>(),
            expected_buildings
        );
        assert_eq!(
            level
                .entities()
                .iter()
                .filter(|entity| {
                    entity.kind() == LevelEntityKind::Building(BuildingKind::House)
                        && entity.position_access() == LevelEntityPositionAccess::SessionReset
                })
                .count(),
            expected_writable_houses
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
            &[(CreatureKind::Goat, 33), (CreatureKind::Villager, 28)],
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
            57,
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
        let level_one_movable = definition
            .level_definition()
            .unwrap()
            .entities()
            .iter()
            .filter(|entity| entity.position_access() == LevelEntityPositionAccess::SessionReset)
            .collect::<Vec<_>>();
        assert_eq!(
            level_one_movable
                .iter()
                .filter(|entity| entity.kind() == LevelEntityKind::Creature(CreatureKind::Goat))
                .count(),
            33
        );
        assert_eq!(
            level_one_movable
                .iter()
                .filter(|entity| {
                    !matches!(
                        entity.kind(),
                        LevelEntityKind::Creature(CreatureKind::Goat | CreatureKind::Villager)
                            | LevelEntityKind::Building(BuildingKind::House)
                    )
                })
                .map(|entity| entity.id().as_str())
                .collect::<Vec<_>>(),
            vec![
                "switch.start_area_exit",
                "switch.area_entry_trigger.at_165_211",
                "item.experience_scroll.at_216_213",
                "spell_pickup.castle.at_194_213",
                "switch.area_entry_trigger.at_165_198",
                "spell_pickup.speed.at_202_212",
                "creature.archer.at_202_209",
                "creature.archer.at_202_217",
                "creature.archer.at_195_217",
                "switch.area_entry_trigger.at_198_213",
                "creature.archer.at_195_209",
                "creature.firefly.at_217_126",
                "scenery.exit_portal.at_97_221",
                "switch.area_entry_trigger.at_165_186",
                "creature.firefly.at_213_125",
                "creature.firefly.at_211_132",
                "switch.exit_sequence_trigger.at_75_218",
                "item.experience_scroll.at_214_215",
                "item.experience_scroll.at_214_211",
                "switch.area_entry_trigger.at_172_182",
                "switch.area_entry_trigger.at_185_182",
                "switch.area_entry_trigger.at_196_182",
                "switch.area_entry_trigger.at_208_182",
                "switch.area_entry_trigger.at_221_182",
                "switch.area_entry_trigger.at_233_182",
                "scenery.stone.at_75_218",
                "scenery.tree.at_110_96",
                "scenery.tree.at_102_91",
                "scenery.tree.at_104_101",
                "scenery.tree.at_141_72",
                "scenery.tree.at_145_70",
                "scenery.tree.at_143_63",
                "scenery.tree.at_129_63",
                "scenery.tree.at_117_49",
                "scenery.tree.at_109_37",
                "scenery.tree.at_98_33",
                "scenery.tree.at_37_10",
                "scenery.tree.at_40_237",
                "scenery.tree.at_102_214",
                "scenery.tree.at_72_172",
                "scenery.tree.at_70_170",
                "scenery.tree.at_71_165",
                "scenery.tree.at_67_165",
                "scenery.tree.at_55_185",
                "switch.area_entry_trigger.at_233_194",
                "creature.firefly.at_209_129",
                "creature.firefly.at_215_129",
                "switch.area_entry_trigger.at_233_228",
                "switch.area_entry_trigger.at_233_240",
                "switch.area_entry_trigger.at_221_240",
                "switch.area_entry_trigger.at_209_240",
                "switch.area_entry_trigger.at_196_240",
                "switch.area_entry_trigger.at_185_240",
                "switch.area_entry_trigger.at_172_240",
                "switch.area_entry_trigger.at_165_236",
                "switch.area_entry_trigger.at_165_224",
                "switch.area_entry_trigger.at_234_212",
                "switch.area_exit_trigger.at_189_212",
                "switch.area_entry_trigger.at_199_213",
                "item.experience_scroll.at_218_211",
                "switch.area_entry_trigger.at_116_213",
                "item.experience_scroll.at_218_215",
                "switch.area_entry_trigger.at_217_213",
                "creature.firefly.at_217_210",
                "creature.worm.at_110_97",
                "creature.worm.at_90_37",
                "creature.worm.at_51_32",
                "creature.worm.at_53_147",
            ]
        );
        assert_eq!(
            level_one_movable
                .iter()
                .filter(|entity| {
                    entity.kind() == LevelEntityKind::Creature(CreatureKind::Villager)
                })
                .count(),
            28
        );
        assert_eq!(
            level_one_movable
                .iter()
                .filter(|entity| {
                    entity.kind() == LevelEntityKind::Building(BuildingKind::House)
                })
                .count(),
            57
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
            &[(CreatureKind::Goat, 36), (CreatureKind::Villager, 29)],
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
            13,
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
                ObjectiveCondition::DefeatRivalWizard(
                    sim_definition::adapter::stable_rival_wizard_id("rival_wizard.nyphur").unwrap(),
                ),
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
        let level_two_movable = level_two
            .level_definition()
            .unwrap()
            .entities()
            .iter()
            .filter(|entity| entity.position_access() == LevelEntityPositionAccess::SessionReset)
            .collect::<Vec<_>>();
        assert_eq!(level_two_movable.len(), 197);
        for (kind, expected) in [
            (CreatureKind::Goat, 36),
            (CreatureKind::Archer, 4),
            (CreatureKind::Bee, 12),
            (CreatureKind::Worm, 6),
            (CreatureKind::Skeleton, 6),
            (CreatureKind::Trader, 27),
            (CreatureKind::Villager, 29),
        ] {
            assert_eq!(
                level_two_movable
                    .iter()
                    .filter(|entity| entity.kind() == LevelEntityKind::Creature(kind))
                    .count(),
                expected
            );
        }
        assert_eq!(
            level_two_movable
                .iter()
                .filter(|entity| {
                    entity.kind() == LevelEntityKind::Building(BuildingKind::House)
                })
                .count(),
            13
        );
        assert_eq!(
            level_two_movable
                .iter()
                .filter(|entity| matches!(entity.kind(), LevelEntityKind::SpellPickup(_)))
                .map(|entity| entity.id().as_str())
                .collect::<Vec<_>>(),
            vec![
                "spell_pickup.shield.at_138_219",
                "spell_pickup.invisibility.at_138_237",
                "spell_pickup.beyond_sight.at_1_152",
                "spell_pickup.speed.at_167_59",
            ]
        );
        assert_eq!(
            level_two_movable
                .iter()
                .filter(|entity| {
                    matches!(
                        entity.kind(),
                        LevelEntityKind::Item(ItemKind::ExperienceScroll)
                    )
                })
                .count(),
            24
        );
        assert_eq!(
            level_two_movable
                .iter()
                .filter(|entity| {
                    matches!(entity.kind(), LevelEntityKind::Scenery(SceneryKind::Stone))
                })
                .count(),
            1
        );
        assert_eq!(
            level_two_movable
                .iter()
                .filter(|entity| {
                    matches!(entity.kind(), LevelEntityKind::Scenery(SceneryKind::Tree))
                })
                .count(),
            12
        );
        for (kind, expected) in [
            (SwitchKind::AreaEntryTrigger, 17),
            (SwitchKind::AreaExitTrigger, 4),
            (SwitchKind::ExitSequenceTrigger, 1),
        ] {
            assert_eq!(
                level_two_movable
                    .iter()
                    .filter(|entity| entity.kind() == LevelEntityKind::Switch(kind))
                    .count(),
                expected
            );
        }
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
            &[(CreatureKind::Worm, 5), (CreatureKind::Skeleton, 11)],
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
            49,
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
        let level_three_movable = level_three
            .level_definition()
            .unwrap()
            .entities()
            .iter()
            .filter(|entity| entity.position_access() == LevelEntityPositionAccess::SessionReset)
            .collect::<Vec<_>>();
        assert_eq!(level_three_movable.len(), 173);
        for (kind, expected) in [
            (CreatureKind::Goat, 11),
            (CreatureKind::Archer, 18),
            (CreatureKind::Bee, 6),
            (CreatureKind::Dragon, 5),
            (CreatureKind::Worm, 5),
        ] {
            assert_eq!(
                level_three_movable
                    .iter()
                    .filter(|entity| entity.kind() == LevelEntityKind::Creature(kind))
                    .count(),
                expected
            );
        }
        assert_eq!(
            level_three_movable
                .iter()
                .filter(|entity| {
                    entity.kind() == LevelEntityKind::Building(BuildingKind::House)
                })
                .count(),
            49
        );
        assert_eq!(
            level_three_movable
                .iter()
                .filter(|entity| matches!(entity.kind(), LevelEntityKind::SpellPickup(_)))
                .map(|entity| entity.id().as_str())
                .collect::<Vec<_>>(),
            vec![
                "spell_pickup.shield.at_85_85",
                "spell_pickup.invisibility.at_136_85",
                "spell_pickup.morph.at_110_109",
            ]
        );
        assert_eq!(
            level_three_movable
                .iter()
                .filter(|entity| {
                    matches!(entity.kind(), LevelEntityKind::Scenery(SceneryKind::Stone))
                })
                .count(),
            12
        );
        assert_eq!(
            level_three_movable
                .iter()
                .filter(|entity| {
                    matches!(entity.kind(), LevelEntityKind::Scenery(SceneryKind::Tree))
                })
                .count(),
            37
        );
        assert_eq!(
            level_three_movable
                .iter()
                .filter(|entity| {
                    matches!(
                        entity.kind(),
                        LevelEntityKind::Item(ItemKind::ExperienceScroll)
                    )
                })
                .count(),
            10
        );
        assert_eq!(
            level_three_movable
                .iter()
                .filter(|entity| {
                    matches!(
                        entity.kind(),
                        LevelEntityKind::Switch(SwitchKind::AreaEntryTrigger)
                    )
                })
                .map(|entity| entity.id().as_str())
                .collect::<Vec<_>>(),
            vec![
                "switch.area_entry_trigger.at_110_110",
                "switch.area_entry_trigger.at_218_93",
                "switch.area_entry_trigger.at_249_118",
                "switch.area_entry_trigger.at_57_10",
            ]
        );
        assert_eq!(
            level_three_movable
                .iter()
                .filter(|entity| {
                    entity.kind() == LevelEntityKind::Switch(SwitchKind::ExitSequenceTrigger)
                })
                .map(|entity| entity.id().as_str())
                .collect::<Vec<_>>(),
            vec!["switch.exit_sequence_trigger.at_213_220"]
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
                [true, true, true, true, true],
                &["archer", "firefly"][..],
                false,
                false,
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
                [true, false, true, true, false],
                &["bee", "archer", "skeleton"][..],
                true,
                true,
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
                [true, true, true, true, true],
                &["dragon", "bee", "archer", "skeleton"][..],
                false,
                false,
            ),
        ];
        let mut writable = 0;
        for (
            root,
            expected_ids,
            expected_writable,
            expected_no_living_kinds,
            expects_standing_building_capability,
            expects_rival_wizard_capability,
        ) in cases
        {
            let definition = load_mc2_campaign(root, &catalog).unwrap();
            assert!(
                definition
                    .required_capabilities()
                    .iter()
                    .any(|requirement| {
                        requirement.id.as_str() == "engine.objective.no_living_creatures"
                            && requirement.exact_version == 1
                    })
            );
            assert_eq!(
                definition
                    .required_capabilities()
                    .iter()
                    .any(|requirement| {
                        requirement.id.as_str() == "engine.objective.no_standing_buildings"
                            && requirement.exact_version == 1
                    }),
                expects_standing_building_capability
            );
            assert_eq!(
                definition
                    .required_capabilities()
                    .iter()
                    .any(|requirement| {
                        requirement.id.as_str() == "engine.objective.defeat_rival_wizard"
                            && requirement.exact_version == 1
                    }),
                expects_rival_wizard_capability
            );
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
                if matches!(
                    objective.condition(),
                    ObjectiveCondition::NoLivingCreatures(_)
                ) {
                    let target =
                        SemanticId::parse(&format!("{}.condition", objective.id().as_str()))
                            .unwrap();
                    assert_eq!(
                        definition
                            .level_objective_condition_descriptor(&target)
                            .unwrap()
                            .enum_values,
                        expected_no_living_kinds
                    );
                }
                writable += usize::from(expected_writable);
            }
        }
        assert_eq!(writable, 13);
    }

    #[test]
    fn semantic_runtime_flow_binds_the_first_three_packages_without_source_identity() {
        let catalog = default_catalog().unwrap();
        let cases = [
            (
                level_one_descriptor_fixture_path(),
                "level.magic_carpet_2.night_spire",
            ),
            (
                descriptor_fixture_path(),
                "level.magic_carpet_2.payahandra_ruins",
            ),
            (
                level_three_descriptor_fixture_path(),
                "level.magic_carpet_2.revealed_way",
            ),
        ];
        let expected_levels = [
            "level.magic_carpet_2.night_spire",
            "level.magic_carpet_2.payahandra_ruins",
            "level.magic_carpet_2.revealed_way",
        ];
        let mut definitions = Vec::new();
        for (root, expected_bound) in cases {
            let definition = load_mc2_campaign(root, &catalog).unwrap();
            let flow = definition
                .runtime_flow()
                .expect("qualified campaign definitions carry semantic runtime flow");
            assert_eq!(flow.game_id().as_str(), "game.magic_carpet_2");
            assert_eq!(flow.bound_level_id().as_str(), expected_bound);
            assert_eq!(flow.entry_level_id().as_str(), expected_levels[0]);
            assert_eq!(
                flow.entries()
                    .iter()
                    .map(|entry| entry.level_id().as_str())
                    .collect::<Vec<_>>(),
                expected_levels
            );
            assert!(matches!(
                flow.entries()[0].completion_destination(),
                RuntimeFlowDestination::Level { level_id }
                    if level_id.as_str() == expected_levels[1]
            ));
            assert!(matches!(
                flow.entries()[1].completion_destination(),
                RuntimeFlowDestination::Level { level_id }
                    if level_id.as_str() == expected_levels[2]
            ));
            assert!(matches!(
                flow.entries()[2].completion_destination(),
                RuntimeFlowDestination::Title { node_id }
                    if node_id.as_str() == "flow.magic_carpet_2.return_to_title"
            ));
            let public_json = serde_json::to_string(flow).unwrap();
            for private in [
                "external_level",
                "internal_level",
                "map_type",
                "package_path",
                "level1",
                "level2",
                "level3",
                "source_slot",
            ] {
                assert!(!public_json.contains(private), "{private}: {public_json}");
            }
            definitions.push(definition);
        }

        let base = &definitions[0];
        let mut rebound_json = serde_json::to_value(base.runtime_flow().unwrap()).unwrap();
        rebound_json["bound_level_id"] = serde_json::json!("level.magic_carpet_2.payahandra_ruins");
        let rebound_flow: RuntimeFlowDefinition = serde_json::from_value(rebound_json).unwrap();
        let rebound = with_level_and_runtime_flow(
            base.clone(),
            base.level_definition().unwrap().clone(),
            rebound_flow,
        );
        assert_ne!(rebound.definition_hash(), base.definition_hash());
        assert!(rebound.verify_semantic_variant_of(base, &catalog).is_err());

        let mut skipped_json = serde_json::to_value(base.runtime_flow().unwrap()).unwrap();
        skipped_json["entries"][0]["completion_destination"]["level_id"] =
            serde_json::json!("level.magic_carpet_2.revealed_way");
        assert!(serde_json::from_value::<RuntimeFlowDefinition>(skipped_json).is_err());

        // No-Claim: this binds one immutable three-level semantic catalog and
        // the currently selected level. It does not consume completion, find
        // the next package, execute transitions/results, or prove save/replay
        // and client presentation.
    }

    fn guard(session: &GameSession<Mc2CampaignBackend>) -> SessionGuard {
        let identity = session.identity();
        SessionGuard {
            session_id: identity.session_id,
            generation: identity.generation,
            expected_tick: identity.tick,
        }
    }

    fn stage_castle_mana_objective_percent(
        session: &mut GameSession<Mc2CampaignBackend>,
        stage_index: usize,
        target_percent: i32,
    ) -> (i32, usize) {
        let compatibility = session.backend_mut().campaign.level2_compatibility_mut();
        let player_count = usize::from(compatibility.session.state.live.core.runtime.player_count);
        let (objective_index, player_entity_index) = compatibility
            .session
            .state
            .live
            .core
            .players
            .iter()
            .take(player_count)
            .find(|player| player.active_player_index >= 0)
            .map(|player| {
                (
                    usize::from(player.player_color_index()),
                    player.entity_index,
                )
            })
            .expect("the package exposes one participating objective owner");
        let objective =
            &mut compatibility.session.state.live.core.runtime.objectives[objective_index];
        objective.done = 0;
        objective.is_level_end = 0;
        objective.stage_selectors.fill(0);
        objective.stage_selectors[stage_index] = 1;
        objective.text_selector = stage_index as u8;
        compatibility
            .activate_authored_group_prerequisite_for_test(5)
            .expect("the real objective gate group materializes");
        let player_position = compatibility.state.position;
        let castle_slot = compatibility
            .economy_mut()
            .build_castle(player_position, player_entity_index)
            .expect("the package arena can stage the Castle objective prerequisite");
        let denominator = compatibility.economy().owner_mana_global();
        let existing = compatibility
            .economy()
            .owner_mana_aggregate(player_entity_index);
        assert!(denominator > 0);
        let target_total = denominator
            .saturating_mul(target_percent)
            .saturating_add(99)
            / 100;
        assert!(target_total >= existing);
        assert!(
            compatibility
                .economy_mut()
                .set_castle_mana_prerequisite_for_test(castle_slot, target_total - existing)
        );
        let economy = compatibility.economy();
        let castle = economy.castle().unwrap();
        (
            100_i32
                * economy
                    .owner_mana_aggregate(player_entity_index)
                    .saturating_add(castle.mana)
                / economy.owner_mana_global(),
            objective_index,
        )
    }

    fn set_staged_castle_mana_percent(
        session: &mut GameSession<Mc2CampaignBackend>,
        target_percent: i32,
    ) -> i32 {
        let compatibility = session.backend_mut().campaign.level2_compatibility_mut();
        let player_count = usize::from(compatibility.session.state.live.core.runtime.player_count);
        let player_entity_index = compatibility
            .session
            .state
            .live
            .core
            .players
            .iter()
            .take(player_count)
            .find(|player| player.active_player_index >= 0)
            .map(|player| player.entity_index)
            .expect("the package exposes one participating objective owner");
        let castle_slot = compatibility.economy().player_castle_entity_index();
        let denominator = compatibility.economy().owner_mana_global();
        let existing = compatibility
            .economy()
            .owner_mana_aggregate(player_entity_index);
        let target_total = denominator
            .saturating_mul(target_percent)
            .saturating_add(99)
            / 100;
        assert!(target_total >= existing);
        assert!(
            compatibility
                .economy_mut()
                .set_castle_mana_prerequisite_for_test(castle_slot, target_total - existing)
        );
        let economy = compatibility.economy();
        let castle = economy.castle().unwrap();
        100_i32
            * economy
                .owner_mana_aggregate(player_entity_index)
                .saturating_add(castle.mana)
            / economy.owner_mana_global()
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
    fn campaign_backend_binds_package_present_goat_policy_across_reopen_and_reset() {
        let catalog = default_catalog().unwrap();
        let policy_id = SemanticId::parse(ENEMY_DEFAULT_POLICY_ID).unwrap();
        let mut expected_vigilant = GoatBehaviorConfig::LEVEL_ONE;
        expected_vigilant.perception_cadence = 1;
        expected_vigilant.perception_radius = u16::MAX;
        expected_vigilant.perception_half_angle = 1_025;
        expected_vigilant.behavior_flags |= 0x10;

        for (ordinal, root, expected_goats) in [
            (0_u64, level_one_descriptor_fixture_path(), 33_usize),
            (1_u64, descriptor_fixture_path(), 36_usize),
            (2_u64, level_three_descriptor_fixture_path(), 11_usize),
        ] {
            let base = load_mc2_campaign(&root, &catalog).unwrap();
            assert_eq!(
                base.value(&policy_id),
                Some(&SemanticValue::Enum("classic_goat_v1".to_owned()))
            );
            assert_eq!(
                base.semantic_descriptor(&catalog, &policy_id)
                    .unwrap()
                    .access,
                Access::Writable
            );
            assert_eq!(
                base.level_definition()
                    .unwrap()
                    .entities()
                    .iter()
                    .filter(|entity| {
                        entity.kind() == LevelEntityKind::Creature(CreatureKind::Goat)
                    })
                    .count(),
                expected_goats
            );

            let workspace_root =
                temporary_workspace_path(&format!("campaign-goat-policy-{ordinal}"));
            let store = WorkspaceStore::create(
                &workspace_root,
                "campaign-goat-policy-authoring",
                base.clone(),
            )
            .unwrap();
            let mut old = GameSession::open(
                &Mc2CampaignAdapter,
                Arc::new(base.clone()),
                80 + ordinal * 2,
                Mc2CampaignInputs::new(&root),
            )
            .unwrap_or_else(|error| {
                panic!("qualified campaign {ordinal} failed to open: {error:?}")
            });
            let mut authored = GameSession::open(
                &Mc2CampaignAdapter,
                Arc::new(base.clone()),
                81 + ordinal * 2,
                Mc2CampaignInputs::new(&root),
            )
            .unwrap();
            let vigilant = publish_enemy_goat_policy(
                &store,
                &catalog,
                &base,
                &format!("publish-vigilant-goat-policy-{ordinal}"),
                "vigilant_goat_v1",
            );

            assert_eq!(old.identity().definition_hash, base.definition_hash());
            assert_eq!(
                old.backend()
                    .campaign
                    .level2_compatibility()
                    .goat_behavior_config(),
                GoatBehaviorConfig::LEVEL_ONE
            );
            let reopened = authored
                .reopen(guard(&authored), Arc::new(vigilant.clone()))
                .unwrap();
            assert_eq!(reopened.identity.generation, 2);
            assert_eq!(reopened.identity.definition_revision, 2);
            assert_eq!(
                authored
                    .backend()
                    .campaign
                    .level2_compatibility()
                    .goat_behavior_config(),
                expected_vigilant
            );
            let authored_frame_zero = authored.snapshot().value;
            authored.reset(guard(&authored)).unwrap();
            assert_eq!(authored.snapshot().value, authored_frame_zero);
            assert_eq!(
                authored
                    .backend()
                    .campaign
                    .level2_compatibility()
                    .goat_behavior_config(),
                expected_vigilant
            );

            if ordinal == 0 {
                old.step(guard(&old), GameCommand::default()).unwrap();
                authored
                    .step(guard(&authored), GameCommand::default())
                    .unwrap();
                let old_campaign = &old.backend().campaign;
                let authored_campaign = &authored.backend().campaign;
                let engagements = |campaign: &CampaignSession| {
                    campaign
                        .events()
                        .iter()
                        .filter_map(|event| match event {
                            Level2DispatchEvent::Goat { events, .. } => Some(events.as_ref()),
                            _ => None,
                        })
                        .flatten()
                        .filter(|event| {
                            matches!(event, GoatStepEvent::EngagementBoundaryReached { .. })
                        })
                        .count()
                };
                assert_eq!(engagements(old_campaign), 0);
                let authored_engagements = engagements(authored_campaign);
                assert!(authored_engagements > 0);

                let old_snapshot = old.snapshot().value;
                let authored_snapshot = authored.snapshot().value;
                assert_eq!(authored_snapshot.tick, old_snapshot.tick);
                assert_eq!(authored_snapshot.terrain_hash, old_snapshot.terrain_hash);
                assert_eq!(authored_snapshot.player_pose, old_snapshot.player_pose);
                assert_eq!(authored_snapshot.player_motion, old_snapshot.player_motion);
                assert_eq!(
                    authored_campaign.level2_compatibility().source_global_rng(),
                    old_campaign.level2_compatibility().source_global_rng()
                );

                let player_entity_index = authored_campaign
                    .level2_compatibility()
                    .shared_spells()
                    .players()[0]
                    .entity_index;
                let old_goats = old_campaign.level2_compatibility().goats();
                let authored_goats = authored_campaign.level2_compatibility().goats();
                let mut changed = 0;
                for (old_goat, authored_goat) in old_goats.iter().zip(authored_goats) {
                    if old_goat == authored_goat {
                        continue;
                    }
                    changed += 1;
                    assert_eq!(authored_goat.action, 14);
                    assert_eq!(authored_goat.stage_var_2, 10);
                    assert_eq!(authored_goat.target_entity_index, Some(player_entity_index));
                    assert_eq!(authored_goat.rng, old_goat.rng);
                    let mut normalized = authored_goat.clone();
                    normalized.action = old_goat.action;
                    normalized.stage_var_2 = old_goat.stage_var_2;
                    normalized.target_entity_index = old_goat.target_entity_index;
                    normalized.actual_speed = old_goat.actual_speed;
                    assert_eq!(&normalized, old_goat);
                }
                assert_eq!(changed, authored_engagements);

                let authored_after_step = authored.snapshot().value;
                authored.reset(guard(&authored)).unwrap();
                authored
                    .step(guard(&authored), GameCommand::default())
                    .unwrap();
                assert_eq!(authored.snapshot().value, authored_after_step);
            }

            std::fs::remove_dir_all(workspace_root).unwrap();
        }

        // No-Claim: this proves the package-present Goat policy binding for
        // the three qualified definitions and one exact ordinary Level-1
        // engagement decision through publication, reopen, and reset. It
        // does not author Goat placement/population, damage/death, movement
        // parameters, other creature families, or prove an organic campaign.
    }

    #[test]
    fn campaign_backend_binds_real_reset_goat_max_life_before_ordinary_damage_after_reopen_and_reset()
     {
        const PACKAGE_MAX_LIFE: u32 = 600;
        const AUTHORED_MAX_LIFE: u32 = 1_200;
        const DAMAGE: i32 = 700;

        let root = level_one_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let goat = sim_definition::adapter::stable_entity_id("creature.goat.at_110_38").unwrap();
        let target = base.level_definition().unwrap().entity(&goat).unwrap();
        assert_eq!(
            target.creature_health().map(|health| (
                health.default_max_life(),
                health.max_life(),
                health.max_life_access(),
            )),
            Some((
                PACKAGE_MAX_LIFE,
                PACKAGE_MAX_LIFE,
                LevelEntityParameterAccess::SessionReset,
            ))
        );
        assert!(base.required_capabilities().iter().any(|requirement| {
            requirement.id.as_str() == GOAT_MAX_LIFE_CAPABILITY_ID && requirement.exact_version == 1
        }));

        let workspace_root = temporary_workspace_path("campaign-goat-max-life");
        let store = WorkspaceStore::create(&workspace_root, "campaign-goat-max-life", base.clone())
            .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            96,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let goat_slot = session
            .backend()
            .campaign
            .level2_compatibility()
            .goats()
            .iter()
            .find(|candidate| candidate.position.x >> 8 == 110 && candidate.position.y >> 8 == 38)
            .map(|candidate| candidate.entity_index)
            .unwrap();
        let change_set = ChangeSetBuilder::new(
            "campaign-goat-max-life",
            &base,
            "raise-one-level-one-goat-max-life",
            RequestedTiming::SessionReset,
        )
        .set_entity_max_life_from(goat.clone(), PACKAGE_MAX_LIFE, AUTHORED_MAX_LIFE)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();

        let inspect = |session: &GameSession<Mc2CampaignBackend>| {
            let compatibility = session.backend().campaign.level2_compatibility();
            let typed = compatibility
                .goats()
                .iter()
                .find(|candidate| candidate.entity_index == goat_slot)
                .unwrap();
            let raw = &compatibility.entities()[usize::from(goat_slot)];
            (
                typed.max_life,
                typed.life,
                typed.action,
                raw.max_life,
                raw.life,
            )
        };
        let damage = |session: &mut GameSession<Mc2CampaignBackend>| {
            let attacker = session
                .backend()
                .campaign
                .level2_compatibility()
                .state
                .entity_index;
            session
                .backend_mut()
                .campaign
                .level2_compatibility_mut()
                .submit_goat_damage(goat_slot, DAMAGE, attacker)
                .unwrap();
            session
                .step(guard(session), GameCommand::default())
                .unwrap();
        };

        assert_eq!(
            inspect(&session),
            (600, 600, 15, 600, 600),
            "publication must not mutate the already-open Goat authority"
        );
        damage(&mut session);
        let package_after_damage = inspect(&session);
        assert_eq!(package_after_damage.1, -100);
        assert_eq!(package_after_damage.2, 12);

        let reopened = session
            .reopen(guard(&session), Arc::new(published.clone()))
            .unwrap();
        assert_eq!(reopened.identity.generation, 2);
        assert_eq!(reopened.identity.definition_revision, 2);
        assert_eq!(inspect(&session), (1_200, 1_200, 15, 1_200, 1_200));
        let authored_frame_zero = session.snapshot().value;
        damage(&mut session);
        let authored_after_damage = inspect(&session);
        assert_eq!(authored_after_damage.1, 500);
        assert_ne!(authored_after_damage.2, 12);

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.snapshot().value, authored_frame_zero);
        assert_eq!(inspect(&session), (1_200, 1_200, 15, 1_200, 1_200));
        damage(&mut session);
        assert_eq!(inspect(&session).1, 500);

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this proves one reset-live Level 1 Goat's guarded
        // maximum-life publication, old-session isolation, explicit
        // reopen/reset materialization, and ordinary mailbox damage branch.
        // It does not author other Goat parameters, prove an organic attacker,
        // or cover Goat death outputs, objectives, save/replay, or Unity UI.
    }

    #[test]
    fn campaign_backend_moves_real_reset_goat_before_ordinary_visit_after_reopen_and_reset() {
        let package_tile = TilePosition { x: 110, y: 38 };
        let authored_tile = TilePosition { x: 111, y: 38 };

        let root = level_one_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let goat = sim_definition::adapter::stable_entity_id("creature.goat.at_110_38").unwrap();
        let target = base.level_definition().unwrap().entity(&goat).unwrap();
        assert_eq!(target.position(), package_tile);
        assert_eq!(
            target.position_access(),
            LevelEntityPositionAccess::SessionReset
        );

        let workspace_root = temporary_workspace_path("campaign-reset-goat-placement");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-reset-goat-placement",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            97,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let goat_slot = session
            .backend()
            .campaign
            .level2_compatibility()
            .goats()
            .iter()
            .find(|candidate| {
                candidate.position.x >> 8 == package_tile.x
                    && candidate.position.y >> 8 == package_tile.y
            })
            .map(|candidate| candidate.entity_index)
            .unwrap();
        let change_set = ChangeSetBuilder::new(
            "campaign-reset-goat-placement",
            &base,
            "move-one-level-one-reset-goat",
            RequestedTiming::SessionReset,
        )
        .move_entity_from(goat, package_tile, authored_tile)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();

        let positions = |session: &GameSession<Mc2CampaignBackend>| {
            let compatibility = session.backend().campaign.level2_compatibility();
            let typed = compatibility
                .goats()
                .iter()
                .find(|candidate| candidate.entity_index == goat_slot)
                .unwrap();
            let raw = &compatibility.entities()[usize::from(goat_slot)];
            (typed.position, raw.position, typed.phase)
        };
        let package = positions(&session);
        assert_eq!((package.0.x >> 8, package.0.y >> 8), (110, 38));
        assert_eq!(package.0, package.1);

        let reopened = session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        assert_eq!(reopened.identity.generation, 2);
        let authored = positions(&session);
        assert_eq!((authored.0.x >> 8, authored.0.y >> 8), (111, 38));
        assert_eq!(authored.0, authored.1);
        let authored_frame_zero = session.snapshot().value;

        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        let visited = positions(&session);
        assert_ne!(visited.0, authored.0);
        assert_eq!(visited.2, authored.2.wrapping_add(1));
        assert!(
            session
                .backend()
                .campaign
                .events()
                .iter()
                .any(|event| matches!(
                    event,
                    Level2DispatchEvent::Goat { source_slot, .. } if *source_slot == goat_slot
                ))
        );

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.snapshot().value, authored_frame_zero);
        let reset = positions(&session);
        assert_eq!(reset.0, authored.0);
        assert_eq!(reset.0, reset.1);

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this proves one reset-live Level 1 Goat placement crosses
        // guarded publication, explicit reopen, the ordinary typed Goat visit,
        // and reset. It does not prove the other 79 Goat
        // placements, authored-group Goat construction, combat, save/replay,
        // or Unity presentation.
    }

    #[test]
    fn campaign_backend_binds_real_authored_goat_variant_at_level_three_group_five_after_reopen_and_reset()
     {
        const GOAT_RECORD: u16 = 166;
        const PACKAGE_MAX_LIFE: u32 = 600;
        const AUTHORED_MAX_LIFE: u32 = 1_200;
        const GROUP_FIVE_ROWS: &[u16] = &[
            166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182,
            183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194,
        ];
        let package_tile = TilePosition { x: 64, y: 69 };
        let authored_tile = TilePosition { x: 65, y: 69 };

        let root = level_three_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let goat = sim_definition::adapter::stable_entity_id("creature.goat.at_64_69").unwrap();
        let target = base.level_definition().unwrap().entity(&goat).unwrap();
        assert_eq!(target.position(), package_tile);
        assert_eq!(
            target.position_access(),
            LevelEntityPositionAccess::SessionReset
        );
        assert_eq!(
            target.creature_health().map(|health| (
                health.default_max_life(),
                health.max_life(),
                health.max_life_access(),
            )),
            Some((
                PACKAGE_MAX_LIFE,
                PACKAGE_MAX_LIFE,
                LevelEntityParameterAccess::SessionReset,
            ))
        );

        let workspace_root = temporary_workspace_path("campaign-authored-goat-variant");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-authored-goat-variant",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            98,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let change_set = ChangeSetBuilder::new(
            "campaign-authored-goat-variant",
            &base,
            "move-and-strengthen-one-level-three-authored-goat",
            RequestedTiming::SessionReset,
        )
        .move_entity_from(goat.clone(), package_tile, authored_tile)
        .set_entity_max_life_from(goat, PACKAGE_MAX_LIFE, AUTHORED_MAX_LIFE)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();

        let construct = |session: &mut GameSession<Mc2CampaignBackend>| {
            let compatibility = session.backend_mut().campaign.level2_compatibility_mut();
            compatibility
                .activate_authored_group_prerequisite_for_test(5)
                .unwrap();
            assert!(compatibility.dispatch_events().iter().any(|event| matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated {
                    group: 5,
                    authored_indices,
                    ..
                } if authored_indices.as_ref() == GROUP_FIVE_ROWS
            )));
            let source = compatibility
                .economy()
                .authored_class5()
                .iter()
                .find(|source| source.authored_index == GOAT_RECORD)
                .copied()
                .unwrap();
            let typed = compatibility
                .goats()
                .iter()
                .find(|candidate| candidate.entity_index == source.source_entity_index)
                .cloned()
                .unwrap();
            (source, typed)
        };

        let (package_source, package_goat) = construct(&mut session);
        assert_eq!(
            (
                package_source.position.x >> 8,
                package_source.position.y >> 8
            ),
            (package_tile.x, package_tile.y)
        );
        assert_eq!((package_source.max_life, package_source.life), (600, 600));
        assert_eq!((package_goat.max_life, package_goat.life), (600, 600));

        let reopened = session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        assert_eq!(reopened.identity.generation, 2);
        assert_eq!(reopened.identity.definition_revision, 2);
        let authored_frame_zero = session.snapshot().value;
        let (authored_source, authored_goat) = construct(&mut session);
        assert_eq!(
            (
                authored_source.position.x >> 8,
                authored_source.position.y >> 8
            ),
            (authored_tile.x, authored_tile.y)
        );
        assert_eq!(authored_source.position, authored_goat.position);
        assert_eq!(
            (authored_source.max_life, authored_source.life),
            (1_200, 1_200)
        );
        assert_eq!((authored_goat.max_life, authored_goat.life), (1_200, 1_200));
        assert!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .economy()
                .map_chains()
                .is_linked(authored_source.source_entity_index)
        );

        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        let visited = session
            .backend()
            .campaign
            .level2_compatibility()
            .goats()
            .iter()
            .find(|candidate| candidate.entity_index == authored_source.source_entity_index)
            .unwrap();
        assert_eq!(visited.max_life, 1_200);
        assert_eq!(
            visited.phase,
            authored_goat.phase.wrapping_add(1),
            "ordinary Goat dispatch must consume the authored carrier"
        );

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.snapshot().value, authored_frame_zero);
        let (reset_source, reset_goat) = construct(&mut session);
        assert_eq!(reset_source.position, authored_source.position);
        assert_eq!(reset_goat.max_life, 1_200);

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this stages the already-qualified Level 3 group-5
        // boundary and proves one authored Goat's position and maximum life
        // reach its ordinary constructor, typed visit, map link, and reset.
        // It does not prove the other group-5 creatures, organic prior
        // progression, Goat combat/death, save/replay, or Unity presentation.
    }

    #[test]
    fn campaign_backend_binds_real_level_one_fireball_cost_to_activation_and_source_charge() {
        let root = level_one_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        assert!(base.required_capabilities().iter().any(|requirement| {
            requirement.id.as_str() == FIREBALL_MANA_COST_CAPABILITY_ID
                && requirement.exact_version == 1
        }));
        let workspace_root = temporary_workspace_path("campaign-fireball-cost-200");
        let store =
            WorkspaceStore::create(&workspace_root, "campaign-fireball-authoring", base.clone())
                .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            61,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        assert_level_one_fireball_cost_mirrors(&session, 100);

        let authored =
            publish_fireball_mana_cost(&store, &catalog, &base, "publish-fireball-cost-200", 200);
        session
            .step(
                guard(&session),
                GameCommand {
                    cast_left: true,
                    ..GameCommand::default()
                },
            )
            .unwrap();
        assert_eq!(player_spell_projectile_count(&session), 1);
        let base_projectiles = session
            .backend()
            .campaign
            .level2_compatibility()
            .economy()
            .fireball_projectiles()
            .to_vec();
        let base_rng = session
            .backend()
            .campaign
            .level2_compatibility()
            .source_global_rng();
        let base_effects = session
            .backend()
            .campaign
            .level2_compatibility()
            .effect_journal()
            .records()
            .to_vec();
        assert_eq!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .shared_spells()
                .player_entity()
                .mana_regen,
            -100,
            "the already-open session must retain its package-default cost"
        );

        let reopened = session
            .reopen(guard(&session), Arc::new(authored.clone()))
            .unwrap();
        assert_eq!(
            (
                reopened.identity.generation,
                reopened.identity.definition_revision
            ),
            (2, 2)
        );
        assert_level_one_fireball_cost_mirrors(&session, 200);
        let authored_frame_zero = session.snapshot().value;
        session
            .step(
                guard(&session),
                GameCommand {
                    cast_left: true,
                    ..GameCommand::default()
                },
            )
            .unwrap();
        assert_eq!(player_spell_projectile_count(&session), 1);
        assert_eq!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .economy()
                .fireball_projectiles(),
            base_projectiles.as_slice(),
            "mana cost must not rewrite projectile identity or trajectory"
        );
        assert_eq!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .source_global_rng(),
            base_rng,
            "mana cost must not alter the accepted-cast RNG schedule"
        );
        assert_eq!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .effect_journal()
                .records(),
            base_effects.as_slice(),
            "mana cost must not alter unrelated accepted-cast effects"
        );
        assert_eq!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .shared_spells()
                .player_entity()
                .mana_regen,
            -200
        );

        session.reset(guard(&session)).unwrap();
        assert_level_one_fireball_cost_mirrors(&session, 200);
        assert_eq!(player_spell_projectile_count(&session), 0);
        assert_eq!(session.snapshot().value, authored_frame_zero);
        std::fs::remove_dir_all(workspace_root).unwrap();
        // No-Claim: this proves reset-time cost binding for the already-enabled
        // local Level 1 tier-zero holder through affordability, first-pulse
        // source charge, and projectile construction. It does not author
        // higher tiers, pickup/acquisition, enemy holders, impact/damage,
        // presentation, save migration, or Level 2/3 Fireball ownership.
    }

    #[test]
    fn campaign_backend_reopens_real_level_one_with_package_retained_fireball_presentation_asset() {
        const SLOT: &str = "presentation.spell.fireball.projectile_asset";
        const BASE: &str = "presentation.fireball.projectile.base";
        const FADE: &str = "presentation.fireball.projectile.fade";
        const BASE_SHA: &str =
            "sha256:331df08c5fa8483d5dd75dc10abd9dd76cc26918d6447403b3f3199bc31f521d";
        const FADE_SHA: &str =
            "sha256:61059124cba22dd3086baab36bb7d12a9ab28ebf37d394827a49bea4f6c96784";

        let root = level_one_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let slot = SemanticId::parse(SLOT).unwrap();
        let descriptor = base.semantic_descriptor(&catalog, &slot).unwrap();
        assert_eq!(descriptor.access, sim_definition::Access::Writable);
        let compatible = base.compatible_presentation_artifacts(&slot).unwrap();
        assert_eq!(
            compatible
                .iter()
                .map(|artifact| (
                    artifact.artifact_id.as_str(),
                    artifact.content_address.as_str()
                ))
                .collect::<Vec<_>>(),
            vec![(BASE, BASE_SHA), (FADE, FADE_SHA)]
        );
        let base_asset = base.presentation_asset(&slot).unwrap();
        assert_eq!(base_asset.content_address, BASE_SHA);
        assert_eq!((base_asset.asset.width, base_asset.asset.height), (34, 35));
        assert_eq!(base_asset.asset.frame_count, 1);

        let workspace_root = temporary_workspace_path("campaign-fireball-presentation");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-fireball-presentation-authoring",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            63,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let frame_zero = session.snapshot().value;
        let published = publish_fireball_projectile_asset(&store, &catalog, &base, FADE);
        assert_eq!(published.package_graph_hash(), base.package_graph_hash());
        assert_eq!(published.artifact_graph_hash(), base.artifact_graph_hash());
        assert_ne!(published.definition_hash(), base.definition_hash());

        let old_observation = session.presentation_asset(guard(&session), &slot).unwrap();
        assert_eq!(old_observation.value.content_address, BASE_SHA);
        assert_eq!(session.snapshot().value, frame_zero);

        let reopened = session
            .reopen(guard(&session), Arc::new(published.clone()))
            .unwrap();
        assert_eq!(
            (reopened.identity.generation, reopened.identity.tick),
            (2, 0)
        );
        assert_eq!(reopened.value, frame_zero);
        let authored = session.presentation_asset(guard(&session), &slot).unwrap();
        assert_eq!(authored.value.content_address, FADE_SHA);
        assert_eq!(authored.value.asset.indices, base_asset.asset.indices);
        assert_ne!(
            authored.value.asset.palette_rgba8,
            base_asset.asset.palette_rgba8
        );
        assert_eq!(session.snapshot().value, frame_zero);

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.snapshot().value, frame_zero);
        let reset_asset = session.presentation_asset(guard(&session), &slot).unwrap();
        assert_eq!(reset_asset.value, authored.value);

        let restored = reissue_at_revision(&base, 3).unwrap();
        session.reopen(guard(&session), Arc::new(restored)).unwrap();
        assert_eq!(
            session
                .presentation_asset(guard(&session), &slot)
                .unwrap()
                .value
                .content_address,
            BASE_SHA
        );
        assert_eq!(session.snapshot().value, frame_zero);
        std::fs::remove_dir_all(workspace_root).unwrap();
        // No-Claim: this proves one qualified Level 1 one-frame Fireball
        // presentation slot, one package-retained palette alternative,
        // package-reload publication, explicit reopen, observational query,
        // reset retention, and base-definition restoration. It does not prove
        // animation, rendering, projectile gameplay changes, artifact import,
        // Level 2/3 presentation, Unity consumption, saves, or distribution.
    }

    #[test]
    fn campaign_snapshot_projects_live_fireball_through_the_bound_semantic_asset() {
        const SLOT: &str = "presentation.spell.fireball.projectile_asset";
        const BASE_SHA: &str =
            "sha256:331df08c5fa8483d5dd75dc10abd9dd76cc26918d6447403b3f3199bc31f521d";
        const FADE_SHA: &str =
            "sha256:61059124cba22dd3086baab36bb7d12a9ab28ebf37d394827a49bea4f6c96784";

        let root = level_one_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let workspace_root = temporary_workspace_path("campaign-fireball-presentation-entity");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-fireball-presentation-authoring",
            base.clone(),
        )
        .unwrap();
        let authored = publish_fireball_projectile_asset(
            &store,
            &catalog,
            &base,
            "presentation.fireball.projectile.fade",
        );
        let mut baseline = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base),
            64,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let mut variant = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(authored),
            65,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        assert!(baseline.snapshot().value.presentation_entities.is_empty());
        assert!(variant.snapshot().value.presentation_entities.is_empty());

        let command = GameCommand {
            cast_left: true,
            ..GameCommand::default()
        };
        let baseline_snapshot = baseline.step(guard(&baseline), command).unwrap().value;
        let variant_snapshot = variant.step(guard(&variant), command).unwrap().value;
        assert_eq!(baseline_snapshot.state_hash, variant_snapshot.state_hash);
        assert_eq!(
            baseline_snapshot.terrain_hash,
            variant_snapshot.terrain_hash
        );
        assert_eq!(
            baseline_snapshot.effect_journal_sha256,
            variant_snapshot.effect_journal_sha256
        );
        let baseline_entity = baseline_snapshot
            .presentation_entities
            .first()
            .expect("ordinary cast projects one live Fireball");
        let variant_entity = variant_snapshot
            .presentation_entities
            .first()
            .expect("authored asset retains the same live Fireball");
        assert_eq!(baseline_snapshot.presentation_entities.len(), 1);
        assert_eq!(variant_snapshot.presentation_entities.len(), 1);
        assert_eq!(baseline_entity.kind.as_str(), "spell.fireball.projectile");
        assert_eq!(baseline_entity.logical_height, FIREBALL_LOGICAL_HEIGHT);
        assert_eq!(baseline_entity.asset.id, SLOT);
        assert_eq!(baseline_entity.asset.content_address, BASE_SHA);
        assert_eq!(variant_entity.asset.content_address, FADE_SHA);
        let source = &baseline
            .backend()
            .campaign
            .level2_compatibility()
            .economy()
            .fireball_projectiles()[0];
        assert_eq!(
            baseline_entity.pose,
            PresentationPose {
                x: source.position.x,
                y: source.position.y,
                z: source.position.z,
                yaw: source.yaw,
                pitch: source.pitch,
            }
        );
        assert_eq!(baseline_entity.actor, variant_entity.actor);
        assert_eq!(baseline_entity.pose, variant_entity.pose);
        assert_eq!(baseline_entity.kind, variant_entity.kind);
        assert_eq!(
            baseline_entity.logical_height,
            variant_entity.logical_height
        );

        variant.reset(guard(&variant)).unwrap();
        assert!(variant.snapshot().value.presentation_entities.is_empty());
        assert_eq!(
            variant
                .presentation_asset(guard(&variant), &SemanticId::parse(SLOT).unwrap())
                .unwrap()
                .value
                .content_address,
            FADE_SHA
        );
        std::fs::remove_dir_all(workspace_root).unwrap();
        // No-Claim: this proves the real Level-1 ordinary Fireball is projected
        // as one opaque semantic presentation entity whose asset follows the
        // immutable definition binding. It does not render Unity pixels,
        // animate frames, project impacts, or support Level 2/3 assets.
    }

    #[test]
    fn campaign_snapshot_projects_authoritative_player_vitals_through_fireball_and_reset() {
        let root = level_one_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let definition = load_mc2_campaign(&root, &catalog).unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(definition),
            66,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();

        let initial = session.snapshot().value.player_vitals;
        let shared = session
            .backend()
            .campaign
            .level2_compatibility()
            .shared_spells();
        assert_eq!(
            initial,
            PlayerVitals {
                life: shared.player_entity().life,
                max_life: shared.player_entity().max_life,
                mana: shared.player_mana(),
                max_mana: shared.player_entity().max_mana,
            }
        );
        assert!(initial.life > 0);
        assert_eq!(initial.life, initial.max_life);
        assert_eq!(initial.mana, initial.max_mana);

        session
            .step(
                guard(&session),
                GameCommand {
                    cast_left: true,
                    ..GameCommand::default()
                },
            )
            .unwrap();
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        let charged = session.snapshot().value.player_vitals;
        assert_eq!(charged.life, initial.life);
        assert_eq!(charged.max_life, initial.max_life);
        assert_eq!(charged.max_mana, initial.max_mana);
        assert_eq!(charged.mana, initial.mana - 100);

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.snapshot().value.player_vitals, initial);
    }

    #[test]
    fn campaign_backend_level_one_fireball_cost_rejects_when_authored_above_player_mana() {
        let root = level_one_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let workspace_root = temporary_workspace_path("campaign-fireball-cost-1100");
        let store =
            WorkspaceStore::create(&workspace_root, "campaign-fireball-authoring", base.clone())
                .unwrap();
        let authored = publish_fireball_mana_cost(
            &store,
            &catalog,
            &base,
            "publish-fireball-cost-1100",
            1_100,
        );
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(authored),
            62,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        assert_level_one_fireball_cost_mirrors(&session, 1_100);
        assert_eq!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .shared_spells()
                .player_entity()
                .mana,
            1_000
        );
        let mana_regen_before = session
            .backend()
            .campaign
            .level2_compatibility()
            .shared_spells()
            .player_entity()
            .mana_regen;
        session
            .step(
                guard(&session),
                GameCommand {
                    cast_left: true,
                    ..GameCommand::default()
                },
            )
            .unwrap();
        let compatibility = session.backend().campaign.level2_compatibility();
        assert_eq!(player_spell_projectile_count(&session), 0);
        assert_eq!(
            compatibility.shared_spells().player_entity().mana_regen,
            mana_regen_before,
            "insufficient mana must not publish a source charge"
        );
        assert!(
            compatibility
                .effect_journal()
                .records()
                .iter()
                .any(|record| matches!(
                    record.effect,
                    CampaignEffect::Flight(FlightEvent::PrepareSound {
                        audio_site: crate::MissionAudioSite::NotEnoughMana,
                        ..
                    })
                ))
        );

        session.reset(guard(&session)).unwrap();
        assert_level_one_fireball_cost_mirrors(&session, 1_100);
        assert_eq!(player_spell_projectile_count(&session), 0);
        std::fs::remove_dir_all(workspace_root).unwrap();
        // No-Claim: this is the same real Level 1 activation boundary as the
        // accepted-cost gate. It does not prove arbitrary mana sources,
        // mid-session edits, higher tiers, or other spell families.
    }

    #[test]
    fn campaign_definitions_without_a_local_fireball_holder_reject_cost_publication() {
        let catalog = default_catalog().unwrap();
        for (label, root) in [
            ("level-two", descriptor_fixture_path()),
            ("level-three", level_three_descriptor_fixture_path()),
        ] {
            let base = load_mc2_campaign(&root, &catalog).unwrap();
            assert!(base.required_capabilities().iter().all(|requirement| {
                requirement.id.as_str() != FIREBALL_MANA_COST_CAPABILITY_ID
            }));
            let workspace_root = temporary_workspace_path(label);
            let store = WorkspaceStore::create(
                &workspace_root,
                "campaign-fireball-authoring",
                base.clone(),
            )
            .unwrap();
            let before = store.state_bytes().unwrap();
            let change_set = ChangeSetBuilder::new(
                "campaign-fireball-authoring",
                &base,
                "unsupported-fireball-cost",
                RequestedTiming::SessionReset,
            )
            .set_integer(IntegerParameter::FireballManaCost, 200)
            .build()
            .unwrap();
            let rejected = store.validate(&catalog, change_set).unwrap();
            assert!(!rejected.accepted);
            assert_eq!(rejected.diagnostics[0].code, "DEPENDENCY_FAILED");
            assert_eq!(store.state_bytes().unwrap(), before);
            std::fs::remove_dir_all(workspace_root).unwrap();
        }
        // No-Claim: this proves package-conditional publication refusal only;
        // it does not add Fireball acquisition or ownership to Level 2/3.
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
        let fixed_gate =
            sim_definition::adapter::stable_entity_id("switch.objective_gate.at_194_17").unwrap();
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
                .entity(&fixed_gate)
                .unwrap()
                .position(),
            TilePosition { x: 194, y: 17 }
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
            "reject-position-insensitive-gate",
            RequestedTiming::SessionReset,
        )
        .move_entity(fixed_gate.clone(), TilePosition { x: 193, y: 17 })
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
                .entity(&fixed_gate)
                .unwrap()
                .position(),
            TilePosition { x: 194, y: 17 }
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
    fn campaign_backend_moves_real_dynamic_area_exit_trigger_before_ordinary_overlap_consumption() {
        const TRIGGER_RECORD: u16 = 231;
        const GROUP_TWO_ROWS: &[u16] = &[105, 231];
        const GROUP_EIGHT_ROWS: &[u16] = &[232];

        let root = level_one_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let trigger =
            sim_definition::adapter::stable_entity_id("switch.area_exit_trigger.at_189_212")
                .unwrap();
        let package_tile = TilePosition { x: 189, y: 212 };
        let authored_tile = TilePosition { x: 100, y: 100 };
        let package_entity = base.level_definition().unwrap().entity(&trigger).unwrap();
        assert_eq!(
            package_entity.kind(),
            LevelEntityKind::Switch(SwitchKind::AreaExitTrigger)
        );
        assert_eq!(package_entity.position(), package_tile);
        assert_eq!(
            package_entity.position_access(),
            LevelEntityPositionAccess::SessionReset
        );

        let workspace_root = temporary_workspace_path("campaign-dynamic-area-exit-placement");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-dynamic-area-exit-placement",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            76,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let change_set = ChangeSetBuilder::new(
            "campaign-dynamic-area-exit-placement",
            &base,
            "move-level-one-dynamic-area-exit",
            RequestedTiming::SessionReset,
        )
        .move_entity_from(trigger.clone(), package_tile, authored_tile)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();

        let construct_group_two = |session: &mut GameSession<Mc2CampaignBackend>| {
            let compatibility = session.backend_mut().campaign.level2_compatibility_mut();
            compatibility
                .activate_authored_group_prerequisite_for_test(2)
                .unwrap();
            assert!(compatibility.dispatch_events().iter().any(|event| matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated {
                    group: 2,
                    authored_indices,
                    ..
                } if authored_indices.as_ref() == GROUP_TWO_ROWS
            )));
            compatibility
                .economy()
                .authored_switch_producers()
                .iter()
                .find(|source| source.authored_index == TRIGGER_RECORD)
                .copied()
                .unwrap()
        };

        let package_trigger = construct_group_two(&mut session);
        assert_eq!(
            (
                package_trigger.position.x >> 8,
                package_trigger.position.y >> 8
            ),
            (package_tile.x, package_tile.y)
        );
        assert_eq!(session.identity().definition_hash, base.definition_hash());

        let reopened = session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        assert_eq!(reopened.identity.generation, 2);
        assert_eq!(reopened.identity.definition_revision, 2);
        let compatibility = session.backend().campaign.level2_compatibility();
        assert_eq!(
            compatibility
                .mission()
                .authored_features()
                .iter()
                .find(|feature| feature.index == TRIGGER_RECORD)
                .map(|feature| (feature.tile_x, feature.tile_y)),
            Some((authored_tile.x, authored_tile.y))
        );
        let authored_reset_hash = session.backend().campaign.state_hash();

        let authored_trigger = construct_group_two(&mut session);
        assert_eq!(
            (
                authored_trigger.position.x >> 8,
                authored_trigger.position.y >> 8
            ),
            (authored_tile.x, authored_tile.y)
        );
        assert_ne!(
            (
                authored_trigger.position.x >> 8,
                authored_trigger.position.y >> 8
            ),
            (package_tile.x, package_tile.y)
        );
        let source_slot = authored_trigger.source_entity_index;
        let players = session
            .backend()
            .campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .filter(|entity| {
                entity.active && entity.class == 3 && entity.model == 0 && entity.life >= 0
            })
            .map(|entity| entity.index)
            .collect::<Vec<_>>();
        assert!(!players.is_empty());
        for player in &players {
            session
                .backend_mut()
                .campaign
                .level2_compatibility_mut()
                .set_level2_entity_position(*player, authored_trigger.position)
                .unwrap();
        }
        for _ in 0..8 {
            session
                .step(guard(&session), GameCommand::default())
                .unwrap();
        }
        assert!(
            session
                .backend()
                .campaign
                .events()
                .iter()
                .all(|event| !matches!(
                    event,
                    Level2DispatchEvent::AuthoredGroupActivated {
                        source_slot: actual,
                        group: 8,
                        ..
                    } if *actual == source_slot
                ))
        );

        let outside = crate::Position {
            x: package_tile.x.wrapping_shl(8).wrapping_add(128),
            y: package_tile.y.wrapping_shl(8).wrapping_add(128),
            z: authored_trigger.position.z,
        };
        for player in &players {
            session
                .backend_mut()
                .campaign
                .level2_compatibility_mut()
                .set_level2_entity_position(*player, outside)
                .unwrap();
        }
        for _ in 0..8 {
            session
                .step(guard(&session), GameCommand::default())
                .unwrap();
            if session.backend().campaign.events().iter().any(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::AuthoredGroupActivated {
                        source_slot: actual,
                        group: 8,
                        authored_indices,
                        ..
                    } if *actual == source_slot && authored_indices.as_ref() == GROUP_EIGHT_ROWS
                )
            }) {
                break;
            }
        }
        assert!(
            session
                .backend()
                .campaign
                .events()
                .iter()
                .any(|event| matches!(
                    event,
                    Level2DispatchEvent::AuthoredGroupActivated {
                        source_slot: actual,
                        group: 8,
                        authored_indices,
                        ..
                    } if *actual == source_slot && authored_indices.as_ref() == GROUP_EIGHT_ROWS
                ))
        );
        assert!(
            session
                .backend()
                .campaign
                .events()
                .iter()
                .all(|event| !matches!(
                    event,
                    Level2DispatchEvent::Error { source_slot: actual, .. } if *actual == source_slot
                ))
        );

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.backend().campaign.state_hash(), authored_reset_hash);
        let reset = session.backend().campaign.level2_compatibility();
        assert!(reset.economy().authored_switch_producers().is_empty());
        assert_eq!(
            reset
                .mission()
                .authored_features()
                .iter()
                .find(|feature| feature.index == TRIGGER_RECORD)
                .map(|feature| (feature.tile_x, feature.tile_y)),
            Some((authored_tile.x, authored_tile.y))
        );

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this stages the real Level 1 group-2 boundary and player
        // overlap to prove one dynamically authored AreaExit placement crosses
        // publication, explicit reopen, ordinary construction/cadence, exact
        // group-8 consumption, and reset. It does not re-prove the other switch
        // groups, organic prior progression, save/replay, or Unity presentation.
    }

    #[test]
    fn campaign_backend_moves_real_grouped_worm_and_body_chain_before_ordinary_visit_after_reopen()
    {
        const WORM_RECORD: u16 = 307;
        const GROUP_FOUR_ROWS: &[u16] =
            &[21, 132, 137, 138, 139, 140, 141, 233, 304, 305, 307, 308];
        const AUTHORED_MAP_TILE: u16 = u16::from_le_bytes([111, 97]);

        let root = level_one_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let worm = sim_definition::adapter::stable_entity_id("creature.worm.at_110_97").unwrap();
        let package_tile = TilePosition { x: 110, y: 97 };
        let authored_tile = TilePosition { x: 111, y: 97 };
        let package_entity = base.level_definition().unwrap().entity(&worm).unwrap();
        assert_eq!(
            package_entity.kind(),
            LevelEntityKind::Creature(CreatureKind::Worm)
        );
        assert_eq!(package_entity.position(), package_tile);
        assert_eq!(
            package_entity.position_access(),
            LevelEntityPositionAccess::SessionReset
        );

        let workspace_root = temporary_workspace_path("campaign-grouped-worm-placement");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-grouped-worm-placement",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            81,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let change_set = ChangeSetBuilder::new(
            "campaign-grouped-worm-placement",
            &base,
            "move-level-one-grouped-worm",
            RequestedTiming::SessionReset,
        )
        .move_entity_from(worm.clone(), package_tile, authored_tile)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();

        let construct_group_four = |session: &mut GameSession<Mc2CampaignBackend>| {
            let compatibility = session.backend_mut().campaign.level2_compatibility_mut();
            compatibility
                .activate_authored_group_prerequisite_for_test(4)
                .unwrap();
            assert!(compatibility.dispatch_events().iter().any(|event| matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated {
                    group: 4,
                    authored_indices,
                    ..
                } if authored_indices.as_ref() == GROUP_FOUR_ROWS
            )));
            let root = compatibility
                .economy()
                .authored_class5()
                .iter()
                .find(|source| source.authored_index == WORM_RECORD)
                .copied()
                .unwrap();
            let children = compatibility
                .economy()
                .authored_class5_model3_children()
                .iter()
                .filter(|child| child.parent_entity_index == root.source_entity_index)
                .copied()
                .collect::<Vec<_>>();
            assert_eq!(children.len(), 16);
            (root, children)
        };

        let (package_worm, package_children) = construct_group_four(&mut session);
        assert_eq!(
            (package_worm.position.x >> 8, package_worm.position.y >> 8),
            (package_tile.x, package_tile.y),
            "publication must not mutate an already-open session"
        );
        assert!(
            package_children
                .iter()
                .all(|child| child.position == package_worm.position)
        );
        assert_eq!(session.identity().definition_hash, base.definition_hash());

        let reopened = session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        assert_eq!(reopened.identity.generation, 2);
        assert_eq!(reopened.identity.definition_revision, 2);
        let compatibility = session.backend().campaign.level2_compatibility();
        assert_eq!(
            compatibility
                .mission()
                .authored_features()
                .iter()
                .find(|feature| feature.index == WORM_RECORD)
                .map(|feature| (feature.tile_x, feature.tile_y)),
            Some((authored_tile.x, authored_tile.y))
        );
        let authored_reset_hash = session.backend().campaign.state_hash();

        let (authored_worm, authored_children) = construct_group_four(&mut session);
        assert_eq!(
            (authored_worm.position.x >> 8, authored_worm.position.y >> 8),
            (authored_tile.x, authored_tile.y)
        );
        assert!(
            authored_children
                .iter()
                .all(|child| child.position == authored_worm.position)
        );
        let compatibility = session.backend().campaign.level2_compatibility();
        assert_eq!(
            compatibility
                .economy()
                .map_chains()
                .tile_of(authored_worm.source_entity_index),
            AUTHORED_MAP_TILE
        );
        assert!(authored_children.iter().all(|child| {
            compatibility
                .economy()
                .map_chains()
                .tile_of(child.source_entity_index)
                == AUTHORED_MAP_TILE
        }));

        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        let compatibility = session.backend().campaign.level2_compatibility();
        let visited = compatibility
            .economy()
            .authored_class5()
            .iter()
            .find(|source| source.source_entity_index == authored_worm.source_entity_index)
            .copied()
            .unwrap();
        assert_eq!(
            visited.source_byte_3e,
            authored_worm.source_byte_3e.wrapping_add(2)
        );
        assert_ne!(visited.position, authored_worm.position);
        assert!(visited.active && visited.drawing_enabled);
        assert!(
            compatibility
                .economy()
                .map_chains()
                .is_linked(visited.source_entity_index)
        );
        assert!(
            session
                .backend()
                .campaign
                .events()
                .iter()
                .any(|event| matches!(
                    event,
                    Level2DispatchEvent::DynamicArena {
                        source_slot,
                        kind: crate::native_world::Level2DynamicArenaKind::AuthoredClass5,
                        ..
                    } if *source_slot == visited.source_entity_index
                ))
        );
        assert!(
            session
                .backend()
                .campaign
                .events()
                .iter()
                .all(|event| !matches!(
                    event,
                    Level2DispatchEvent::Error { source_slot, .. }
                        if *source_slot == visited.source_entity_index
                ))
        );

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.backend().campaign.state_hash(), authored_reset_hash);
        let reset = session.backend().campaign.level2_compatibility();
        assert!(reset.economy().authored_class5().is_empty());
        assert_eq!(
            reset
                .mission()
                .authored_features()
                .iter()
                .find(|feature| feature.index == WORM_RECORD)
                .map(|feature| (feature.tile_x, feature.tile_y)),
            Some((authored_tile.x, authored_tile.y))
        );
        let (reset_worm, reset_children) = construct_group_four(&mut session);
        assert_eq!(
            (reset_worm.position.x >> 8, reset_worm.position.y >> 8),
            (authored_tile.x, authored_tile.y)
        );
        assert_eq!(reset_children.len(), 16);
        assert!(
            reset_children
                .iter()
                .all(|child| child.position == reset_worm.position)
        );

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this stages the already-proven real Level 1 group-4
        // boundary to prove one authored Worm placement crosses guarded
        // publication, explicit reopen, its full root/body/map constructor,
        // ordinary action-25 movement, and reset. It does not prove organic
        // prior objectives/group activation, Worm combat/death, the other nine
        // authored Worm placements, Level 3 reset-live Worm relocation,
        // health/population authoring, save/replay, or Unity presentation.
    }

    #[test]
    fn campaign_backend_moves_real_level_three_imported_worm_family_before_ordinary_visit_after_reopen()
     {
        const ROOT_SLOT: u16 = 201;
        const BODY_COUNT: usize = 16;
        const LAST_BODY_SLOT: u16 = 217;
        const AUTHORED_MAP_TILE: u16 = u16::from_le_bytes([151, 19]);

        let root = level_three_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let worm = sim_definition::adapter::stable_entity_id("creature.worm.at_150_19").unwrap();
        let package_tile = TilePosition { x: 150, y: 19 };
        let authored_tile = TilePosition { x: 151, y: 19 };
        let package_entity = base.level_definition().unwrap().entity(&worm).unwrap();
        assert_eq!(
            package_entity.kind(),
            LevelEntityKind::Creature(CreatureKind::Worm)
        );
        assert_eq!(package_entity.position(), package_tile);
        assert_eq!(
            package_entity.position_access(),
            LevelEntityPositionAccess::SessionReset
        );

        let workspace_root = temporary_workspace_path("campaign-imported-worm-placement");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-imported-worm-placement",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            82,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let change_set = ChangeSetBuilder::new(
            "campaign-imported-worm-placement",
            &base,
            "move-one-level-three-imported-worm-family",
            RequestedTiming::SessionReset,
        )
        .move_entity_from(worm, package_tile, authored_tile)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();

        let family = |session: &GameSession<Mc2CampaignBackend>| {
            session.backend().campaign.level2_compatibility().entities()
                [usize::from(ROOT_SLOT)..=usize::from(ROOT_SLOT) + BODY_COUNT]
                .to_vec()
        };
        let package_family = family(&session);
        assert_eq!(package_family.len(), BODY_COUNT + 1);
        assert_eq!(
            (
                package_family[0].class,
                package_family[0].model,
                package_family[0].action,
                package_family[0].source_id,
            ),
            (5, 3, 31, ROOT_SLOT)
        );
        for (offset, body) in package_family[1..].iter().enumerate() {
            assert_eq!(
                (body.class, body.model, body.action, body.source_id),
                (5, 3, 232, ROOT_SLOT)
            );
            let expected_next = if offset + 1 == BODY_COUNT {
                0
            } else {
                ROOT_SLOT + u16::try_from(offset).unwrap() + 2
            };
            assert_eq!(
                u16::from_le_bytes(body.record_bytes()[56..58].try_into().unwrap()),
                expected_next
            );
        }
        assert_eq!(
            (
                package_family[0].position.x >> 8,
                package_family[0].position.y >> 8
            ),
            (package_tile.x, package_tile.y)
        );

        let reopened = session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        assert_eq!(reopened.identity.generation, 2);
        assert_eq!(reopened.identity.definition_revision, 2);
        let authored_family = family(&session);
        for (package, authored) in package_family.iter().zip(&authored_family) {
            assert_eq!(authored.position.x, package.position.x.wrapping_add(1 << 8));
            assert_eq!(authored.position.y, package.position.y);
            assert_eq!(authored.position.z, package.position.z);
        }
        let compatibility = session.backend().campaign.level2_compatibility();
        assert!(authored_family.iter().all(|entity| {
            compatibility.economy().map_chains().tile_of(entity.index) == AUTHORED_MAP_TILE
        }));
        let worm_census = compatibility
            .creature_runtime_census()
            .entries()
            .iter()
            .filter(|identity| identity.root_slot == ROOT_SLOT)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(worm_census.len(), BODY_COUNT + 1);
        assert_eq!(
            worm_census
                .iter()
                .filter(|identity| {
                    identity.role
                        == crate::campaign::source_arena::CreatureRuntimeRole::MultipartPart
                })
                .count(),
            BODY_COUNT
        );
        let authored_frame_zero = session.snapshot().value;

        for _ in 0..4 {
            session
                .step(guard(&session), GameCommand::default())
                .unwrap();
        }
        let visited_family = family(&session);
        assert_ne!(visited_family[0].position, authored_family[0].position);
        assert_ne!(visited_family[1].position, authored_family[1].position);
        for (authored, visited) in authored_family.iter().zip(&visited_family) {
            assert_eq!(visited.phase, authored.phase.wrapping_add(4));
            let expected_tile = u16::from_le_bytes([
                u8::try_from(visited.position.x >> 8).unwrap(),
                u8::try_from(visited.position.y >> 8).unwrap(),
            ]);
            assert_eq!(
                session
                    .backend()
                    .campaign
                    .level2_compatibility()
                    .economy()
                    .map_chains()
                    .tile_of(visited.index),
                expected_tile
            );
        }
        assert!(session.backend().campaign.events().iter().all(|event| {
            !matches!(
                event,
                Level2DispatchEvent::Error { source_slot, .. }
                    if (ROOT_SLOT..=LAST_BODY_SLOT).contains(source_slot)
            )
        }));

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.snapshot().value, authored_frame_zero);
        assert_eq!(family(&session), authored_family);

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this proves one reset-live Level 3 Worm placement crosses
        // guarded publication, old-session isolation, explicit reopen, the
        // exact imported root/body/map relocation, ordinary action-31/232
        // visits, and reset. It does not prove the other four L3 Worm
        // placements, organic combat/death, authored population or health,
        // save/replay, or Unity presentation.
    }

    #[test]
    fn campaign_backend_binds_real_dynamic_and_imported_worm_max_life_before_child_damage_after_reopen_and_reset()
     {
        const DYNAMIC_RECORD: u16 = 307;
        const IMPORTED_ROOT: u16 = 201;
        const BODY_COUNT: usize = 16;
        const PACKAGE_MAX_LIFE: u32 = 9_000;
        const AUTHORED_MAX_LIFE: u32 = 18_000;
        const DAMAGE: i32 = 10_000;

        let catalog = default_catalog().unwrap();

        let level_one_root = level_one_descriptor_fixture_path();
        let level_one_base = load_mc2_campaign(&level_one_root, &catalog).unwrap();
        let dynamic_worm =
            sim_definition::adapter::stable_entity_id("creature.worm.at_110_97").unwrap();
        let dynamic_entity = level_one_base
            .level_definition()
            .unwrap()
            .entity(&dynamic_worm)
            .unwrap();
        assert_eq!(
            dynamic_entity.creature_health().map(|health| (
                health.default_max_life(),
                health.max_life(),
                health.max_life_access(),
            )),
            Some((
                PACKAGE_MAX_LIFE,
                PACKAGE_MAX_LIFE,
                LevelEntityParameterAccess::SessionReset,
            ))
        );
        assert!(
            level_one_base
                .required_capabilities()
                .iter()
                .any(|requirement| {
                    requirement.id.as_str() == WORM_MAX_LIFE_CAPABILITY_ID
                        && requirement.exact_version == 1
                })
        );

        let level_one_workspace = temporary_workspace_path("campaign-dynamic-worm-max-life");
        let level_one_store = WorkspaceStore::create(
            &level_one_workspace,
            "campaign-dynamic-worm-max-life",
            level_one_base.clone(),
        )
        .unwrap();
        let level_one_change = ChangeSetBuilder::new(
            "campaign-dynamic-worm-max-life",
            &level_one_base,
            "raise-one-dynamic-worm-max-life",
            RequestedTiming::SessionReset,
        )
        .set_entity_max_life_from(dynamic_worm, PACKAGE_MAX_LIFE, AUTHORED_MAX_LIFE)
        .build()
        .unwrap();
        let level_one_validation = level_one_store
            .validate(&catalog, level_one_change.clone())
            .unwrap();
        assert!(
            level_one_validation.accepted,
            "{:#?}",
            level_one_validation.diagnostics
        );
        level_one_store
            .apply(
                &catalog,
                level_one_change,
                &level_one_validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let level_one_published = level_one_store.active_definition().unwrap();
        let mut dynamic_session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(level_one_base),
            91,
            Mc2CampaignInputs::new(&level_one_root),
        )
        .unwrap();
        let construct_dynamic = |session: &mut GameSession<Mc2CampaignBackend>| {
            let compatibility = session.backend_mut().campaign.level2_compatibility_mut();
            compatibility
                .activate_authored_group_prerequisite_for_test(4)
                .unwrap();
            let root = compatibility
                .economy()
                .authored_class5()
                .iter()
                .find(|source| source.authored_index == DYNAMIC_RECORD)
                .copied()
                .unwrap();
            let children = compatibility
                .economy()
                .authored_class5_model3_children()
                .iter()
                .filter(|child| child.parent_entity_index == root.source_entity_index)
                .copied()
                .collect::<Vec<_>>();
            assert_eq!(children.len(), BODY_COUNT);
            (root, children)
        };
        let damage_dynamic = |session: &mut GameSession<Mc2CampaignBackend>| {
            let attacker = session
                .backend()
                .campaign
                .level2_compatibility()
                .state
                .entity_index;
            let (_, child_slot) = session
                .backend_mut()
                .campaign
                .level2_compatibility_mut()
                .economy_mut()
                .stage_authored_worm_child_mailbox_for_test(DYNAMIC_RECORD, 12, attacker, DAMAGE)
                .unwrap();
            session
                .step(guard(session), GameCommand::default())
                .unwrap();
            let child = session
                .backend()
                .campaign
                .level2_compatibility()
                .economy()
                .authored_class5_model3_children()
                .iter()
                .find(|child| child.source_entity_index == child_slot)
                .copied()
                .unwrap();
            session
                .step(guard(session), GameCommand::default())
                .unwrap();
            let root = session
                .backend()
                .campaign
                .level2_compatibility()
                .economy()
                .authored_class5()
                .iter()
                .find(|source| source.authored_index == DYNAMIC_RECORD)
                .copied()
                .unwrap();
            (child, root)
        };

        let (package_dynamic, package_children) = construct_dynamic(&mut dynamic_session);
        assert_eq!(
            (package_dynamic.max_life, package_dynamic.life),
            (
                PACKAGE_MAX_LIFE.cast_signed(),
                PACKAGE_MAX_LIFE.cast_signed()
            )
        );
        assert!(package_children.iter().all(|child| {
            (child.max_life, child.life)
                == (
                    PACKAGE_MAX_LIFE.cast_signed(),
                    PACKAGE_MAX_LIFE.cast_signed(),
                )
        }));
        let (package_child_after, package_root_after) = damage_dynamic(&mut dynamic_session);
        assert_eq!(package_child_after.life, -1_000);
        assert_eq!(package_root_after.action, 28);

        dynamic_session
            .reopen(guard(&dynamic_session), Arc::new(level_one_published))
            .unwrap();
        let dynamic_authored_reset_hash = dynamic_session.backend().campaign.state_hash();
        let (authored_dynamic, authored_children) = construct_dynamic(&mut dynamic_session);
        assert_eq!(
            (authored_dynamic.max_life, authored_dynamic.life),
            (
                AUTHORED_MAX_LIFE.cast_signed(),
                AUTHORED_MAX_LIFE.cast_signed()
            )
        );
        assert!(authored_children.iter().all(|child| {
            (child.max_life, child.life)
                == (
                    AUTHORED_MAX_LIFE.cast_signed(),
                    AUTHORED_MAX_LIFE.cast_signed(),
                )
        }));
        let (authored_child_after, authored_root_after) = damage_dynamic(&mut dynamic_session);
        assert_eq!(authored_child_after.life, 8_000);
        assert!(authored_root_after.active && authored_root_after.action != 28);

        dynamic_session.reset(guard(&dynamic_session)).unwrap();
        assert_eq!(
            dynamic_session.backend().campaign.state_hash(),
            dynamic_authored_reset_hash
        );
        let (reset_dynamic, reset_children) = construct_dynamic(&mut dynamic_session);
        assert_eq!(
            (reset_dynamic.max_life, reset_dynamic.life),
            (
                AUTHORED_MAX_LIFE.cast_signed(),
                AUTHORED_MAX_LIFE.cast_signed()
            )
        );
        assert!(reset_children.iter().all(|child| {
            (child.max_life, child.life)
                == (
                    AUTHORED_MAX_LIFE.cast_signed(),
                    AUTHORED_MAX_LIFE.cast_signed(),
                )
        }));

        let level_three_root = level_three_descriptor_fixture_path();
        let level_three_base = load_mc2_campaign(&level_three_root, &catalog).unwrap();
        let imported_worm =
            sim_definition::adapter::stable_entity_id("creature.worm.at_150_19").unwrap();
        let imported_entity = level_three_base
            .level_definition()
            .unwrap()
            .entity(&imported_worm)
            .unwrap();
        assert_eq!(
            imported_entity.creature_health().map(|health| (
                health.default_max_life(),
                health.max_life(),
                health.max_life_access(),
            )),
            Some((
                PACKAGE_MAX_LIFE,
                PACKAGE_MAX_LIFE,
                LevelEntityParameterAccess::SessionReset,
            ))
        );
        assert!(
            level_three_base
                .required_capabilities()
                .iter()
                .any(|requirement| {
                    requirement.id.as_str() == WORM_MAX_LIFE_CAPABILITY_ID
                        && requirement.exact_version == 1
                })
        );

        let level_three_workspace = temporary_workspace_path("campaign-imported-worm-max-life");
        let level_three_store = WorkspaceStore::create(
            &level_three_workspace,
            "campaign-imported-worm-max-life",
            level_three_base.clone(),
        )
        .unwrap();
        let level_three_change = ChangeSetBuilder::new(
            "campaign-imported-worm-max-life",
            &level_three_base,
            "raise-one-imported-worm-max-life",
            RequestedTiming::SessionReset,
        )
        .set_entity_max_life_from(imported_worm, PACKAGE_MAX_LIFE, AUTHORED_MAX_LIFE)
        .build()
        .unwrap();
        let level_three_validation = level_three_store
            .validate(&catalog, level_three_change.clone())
            .unwrap();
        assert!(
            level_three_validation.accepted,
            "{:#?}",
            level_three_validation.diagnostics
        );
        level_three_store
            .apply(
                &catalog,
                level_three_change,
                &level_three_validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let level_three_published = level_three_store.active_definition().unwrap();
        let mut imported_session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(level_three_base),
            92,
            Mc2CampaignInputs::new(&level_three_root),
        )
        .unwrap();
        let imported_family = |session: &GameSession<Mc2CampaignBackend>| {
            session.backend().campaign.level2_compatibility().entities()
                [usize::from(IMPORTED_ROOT)..=usize::from(IMPORTED_ROOT) + BODY_COUNT]
                .to_vec()
        };
        let damage_imported = |session: &mut GameSession<Mc2CampaignBackend>| {
            let attacker = session
                .backend()
                .campaign
                .level2_compatibility()
                .state
                .entity_index;
            let child_slot = session
                .backend_mut()
                .campaign
                .level2_compatibility_mut()
                .stage_imported_worm_child_mailbox_for_test(IMPORTED_ROOT, 0, attacker, DAMAGE)
                .unwrap();
            session
                .step(guard(session), GameCommand::default())
                .unwrap();
            let child = session.backend().campaign.level2_compatibility().entities()
                [usize::from(child_slot)]
            .clone();
            session
                .step(guard(session), GameCommand::default())
                .unwrap();
            let root = session.backend().campaign.level2_compatibility().entities()
                [usize::from(IMPORTED_ROOT)]
            .clone();
            (child, root)
        };

        let package_imported = imported_family(&imported_session);
        assert!(package_imported.iter().all(|row| {
            (row.max_life, row.life)
                == (
                    PACKAGE_MAX_LIFE.cast_signed(),
                    PACKAGE_MAX_LIFE.cast_signed(),
                )
        }));
        let (package_imported_child, package_imported_root) =
            damage_imported(&mut imported_session);
        assert_eq!(package_imported_child.life, -1_000);
        assert_eq!(package_imported_root.action, 28);

        imported_session
            .reopen(guard(&imported_session), Arc::new(level_three_published))
            .unwrap();
        let imported_authored_reset_hash = imported_session.backend().campaign.state_hash();
        let authored_imported = imported_family(&imported_session);
        assert_eq!(authored_imported.len(), BODY_COUNT + 1);
        assert!(authored_imported.iter().all(|row| {
            (row.max_life, row.life)
                == (
                    AUTHORED_MAX_LIFE.cast_signed(),
                    AUTHORED_MAX_LIFE.cast_signed(),
                )
        }));
        let (authored_imported_child, authored_imported_root) =
            damage_imported(&mut imported_session);
        assert_eq!(authored_imported_child.life, 8_000);
        assert!(authored_imported_root.active && authored_imported_root.action != 28);

        imported_session.reset(guard(&imported_session)).unwrap();
        assert_eq!(
            imported_session.backend().campaign.state_hash(),
            imported_authored_reset_hash
        );
        assert!(imported_family(&imported_session).iter().all(|row| {
            (row.max_life, row.life)
                == (
                    AUTHORED_MAX_LIFE.cast_signed(),
                    AUTHORED_MAX_LIFE.cast_signed(),
                )
        }));

        std::fs::remove_dir_all(&level_one_workspace).unwrap();
        std::fs::remove_dir_all(&level_three_workspace).unwrap();
        // No-Claim: this stages one already-proven Level-1 group-4 constructor
        // and one child mailbox in each real carrier. It proves guarded Worm
        // maximum-life publication, old-session isolation, all sixteen copied
        // dynamic and imported body values, explicit reopen/reset, and ordinary
        // child-damage/root-death consumption. It does not prove organic group
        // progression, every Worm instance, targeting/projectiles, population
        // authoring, save/replay, or Unity presentation.
    }

    #[test]
    fn campaign_packages_bind_every_first_three_level_worm_health_descriptor() {
        let catalog = default_catalog().unwrap();
        for (root, expected_worms) in [
            (level_one_descriptor_fixture_path(), 4_usize),
            (descriptor_fixture_path(), 6),
            (level_three_descriptor_fixture_path(), 5),
        ] {
            let definition = load_mc2_campaign(&root, &catalog).unwrap();
            let worms = definition
                .level_definition()
                .unwrap()
                .entities()
                .iter()
                .filter(|entity| {
                    entity.kind()
                        == sim_definition::level_definition::LevelEntityKind::Creature(
                            CreatureKind::Worm,
                        )
                })
                .collect::<Vec<_>>();
            assert_eq!(worms.len(), expected_worms);
            assert!(worms.iter().all(|entity| {
                entity.creature_health().is_some_and(|health| {
                    health.default_max_life() == 9_000
                        && health.max_life() == 9_000
                        && health.max_life_access() == LevelEntityParameterAccess::SessionReset
                })
            }));
            assert!(
                definition
                    .required_capabilities()
                    .iter()
                    .any(|requirement| {
                        requirement.id.as_str() == WORM_MAX_LIFE_CAPABILITY_ID
                            && requirement.exact_version == 1
                    })
            );
        }

        // No-Claim: this is the closed first-three-package Worm descriptor
        // census. The separate causal gate owns one real dynamic and imported
        // child-damage visit; this census does not claim every Worm was
        // constructed, damaged, killed, or observed through a client.
    }

    #[test]
    fn campaign_backend_moves_real_level_two_grouped_trader_before_ordinary_action113_after_reopen()
    {
        const TRADER_RECORD: u16 = 125;

        let root = descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let trader =
            sim_definition::adapter::stable_entity_id("creature.trader.at_57_155").unwrap();
        let package_tile = TilePosition { x: 57, y: 155 };
        let authored_tile = TilePosition { x: 60, y: 156 };
        let package_entity = base.level_definition().unwrap().entity(&trader).unwrap();
        assert_eq!(
            package_entity.kind(),
            LevelEntityKind::Creature(CreatureKind::Trader)
        );
        assert_eq!(package_entity.position(), package_tile);
        assert_eq!(
            package_entity.position_access(),
            LevelEntityPositionAccess::SessionReset
        );

        let workspace_root = temporary_workspace_path("campaign-grouped-trader-placement");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-grouped-trader-placement",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            83,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let change_set = ChangeSetBuilder::new(
            "campaign-grouped-trader-placement",
            &base,
            "move-one-level-two-grouped-trader",
            RequestedTiming::SessionReset,
        )
        .move_entity_from(trader, package_tile, authored_tile)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();

        let construct_group_three = |session: &mut GameSession<Mc2CampaignBackend>| {
            let compatibility = session.backend_mut().campaign.level2_compatibility_mut();
            compatibility
                .activate_authored_group_prerequisite_for_test(3)
                .unwrap();
            let expected_group = (43_u16..=64)
                .chain(std::iter::once(108))
                .chain(125..=151)
                .chain(std::iter::once(439))
                .collect::<Vec<_>>();
            let events = compatibility.dispatch_events();
            assert!(
                events.iter().any(|event| matches!(
                    event,
                    Level2DispatchEvent::AuthoredGroupActivated {
                        group: 3,
                        authored_indices,
                        ..
                    } if authored_indices.as_ref() == expected_group.as_slice()
                )),
                "group-3 Trader activation events: {events:?}"
            );
            compatibility
                .economy()
                .authored_class5()
                .iter()
                .find(|source| source.authored_index == TRADER_RECORD)
                .copied()
                .unwrap()
        };

        let package_trader = construct_group_three(&mut session);
        assert_eq!(
            (
                package_trader.position.x >> 8,
                package_trader.position.y >> 8
            ),
            (package_tile.x, package_tile.y),
            "publication must not mutate the already-open package authority"
        );
        assert_eq!(session.identity().definition_hash, base.definition_hash());

        let reopened = session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        assert_eq!(
            (
                reopened.identity.generation,
                reopened.identity.definition_revision
            ),
            (2, 2)
        );
        let compatibility = session.backend().campaign.level2_compatibility();
        assert_eq!(
            compatibility
                .mission()
                .authored_features()
                .iter()
                .find(|feature| feature.index == TRADER_RECORD)
                .map(|feature| (feature.tile_x, feature.tile_y)),
            Some((authored_tile.x, authored_tile.y))
        );
        let authored_reset_hash = session.backend().campaign.state_hash();

        let authored_trader = construct_group_three(&mut session);
        assert_eq!(
            (
                authored_trader.position.x >> 8,
                authored_trader.position.y >> 8
            ),
            (authored_tile.x, authored_tile.y)
        );
        let source_slot = authored_trader.source_entity_index;
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        let compatibility = session.backend().campaign.level2_compatibility();
        let visited = compatibility
            .economy()
            .authored_class5()
            .iter()
            .find(|source| source.source_entity_index == source_slot)
            .copied()
            .unwrap();
        assert_eq!(
            visited.source_byte_3e,
            authored_trader.source_byte_3e.wrapping_add(1)
        );
        assert_eq!((visited.model, visited.action), (14, 113));
        assert!(visited.active && visited.drawing_enabled);
        assert_eq!(
            compatibility.economy().map_chains().tile_of(source_slot),
            u16::from_le_bytes([
                u8::try_from(visited.position.x >> 8).unwrap(),
                u8::try_from(visited.position.y >> 8).unwrap(),
            ])
        );
        assert!(
            session
                .backend()
                .campaign
                .events()
                .iter()
                .any(|event| matches!(
                    event,
                    Level2DispatchEvent::DynamicArena {
                        source_slot: actual,
                        kind: crate::native_world::Level2DynamicArenaKind::AuthoredClass5,
                        ..
                    } if *actual == source_slot
                ))
        );
        assert!(
            session
                .backend()
                .campaign
                .events()
                .iter()
                .all(|event| !matches!(
                    event,
                    Level2DispatchEvent::Error {
                        source_slot: actual,
                        ..
                    } if *actual == source_slot
                ))
        );

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.backend().campaign.state_hash(), authored_reset_hash);
        assert!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .economy()
                .authored_class5()
                .is_empty()
        );
        let reset_trader = construct_group_three(&mut session);
        assert_eq!(
            (reset_trader.position.x >> 8, reset_trader.position.y >> 8),
            (authored_tile.x, authored_tile.y)
        );

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this stages the already-proven Level 2 group-3 boundary
        // and proves one grouped Trader placement crosses guarded publication,
        // old-session isolation, explicit reopen, the real constructor,
        // ordinary action-113 scheduling/map membership, and reset. It does
        // not re-prove organic groups 1-3, Trader targeting/combat/death, the
        // other twenty-six placements, Villager placement, save/replay, or
        // Unity presentation.
    }

    #[test]
    fn campaign_backend_binds_real_level_two_grouped_trader_max_life_before_action113_damage_after_reopen_and_reset()
     {
        const TRADER_RECORD: u16 = 125;
        const PACKAGE_MAX_LIFE: u32 = 1_000;
        const AUTHORED_MAX_LIFE: u32 = 2_000;
        const DAMAGE: i32 = 1_500;

        let root = descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let trader =
            sim_definition::adapter::stable_entity_id("creature.trader.at_57_155").unwrap();
        let package_entity = base.level_definition().unwrap().entity(&trader).unwrap();
        assert_eq!(
            package_entity.creature_health().map(|health| (
                health.default_max_life(),
                health.max_life(),
                health.max_life_access(),
            )),
            Some((
                PACKAGE_MAX_LIFE,
                PACKAGE_MAX_LIFE,
                LevelEntityParameterAccess::SessionReset,
            ))
        );

        let workspace_root = temporary_workspace_path("campaign-trader-max-life");
        let store =
            WorkspaceStore::create(&workspace_root, "campaign-trader-max-life", base.clone())
                .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            84,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let change_set = ChangeSetBuilder::new(
            "campaign-trader-max-life",
            &base,
            "raise-one-level-two-trader-max-life",
            RequestedTiming::SessionReset,
        )
        .set_entity_max_life_from(trader, PACKAGE_MAX_LIFE, AUTHORED_MAX_LIFE)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();

        let construct_group_three = |session: &mut GameSession<Mc2CampaignBackend>| {
            let compatibility = session.backend_mut().campaign.level2_compatibility_mut();
            compatibility
                .activate_authored_group_prerequisite_for_test(3)
                .unwrap();
            compatibility
                .economy()
                .authored_class5()
                .iter()
                .find(|source| source.authored_index == TRADER_RECORD)
                .copied()
                .unwrap()
        };
        let stage_damage = |session: &mut GameSession<Mc2CampaignBackend>, source_slot| {
            assert!(
                session
                    .backend_mut()
                    .campaign
                    .level2_compatibility_mut()
                    .economy_mut()
                    .stage_trader_mailbox_damage_for_test(source_slot, 1, DAMAGE)
            );
        };
        let trader_after_step = |session: &GameSession<Mc2CampaignBackend>, source_slot| {
            session
                .backend()
                .campaign
                .level2_compatibility()
                .economy()
                .authored_class5()
                .iter()
                .find(|source| source.source_entity_index == source_slot)
                .copied()
                .unwrap()
        };

        let package_trader = construct_group_three(&mut session);
        assert_eq!(
            (package_trader.max_life, package_trader.life),
            (
                PACKAGE_MAX_LIFE.cast_signed(),
                PACKAGE_MAX_LIFE.cast_signed()
            )
        );
        stage_damage(&mut session, package_trader.source_entity_index);
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        let package_after = trader_after_step(&session, package_trader.source_entity_index);
        assert_eq!((package_after.life, package_after.action), (-500, 116));
        assert_eq!(session.identity().definition_hash, base.definition_hash());

        let reopened = session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        assert_eq!(
            (
                reopened.identity.generation,
                reopened.identity.definition_revision
            ),
            (2, 2)
        );
        let authored_reset_hash = session.backend().campaign.state_hash();
        let authored_trader = construct_group_three(&mut session);
        assert_eq!(
            (authored_trader.max_life, authored_trader.life),
            (
                AUTHORED_MAX_LIFE.cast_signed(),
                AUTHORED_MAX_LIFE.cast_signed()
            )
        );
        stage_damage(&mut session, authored_trader.source_entity_index);
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        let authored_after = trader_after_step(&session, authored_trader.source_entity_index);
        assert_eq!((authored_after.life, authored_after.action), (500, 118));

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.backend().campaign.state_hash(), authored_reset_hash);
        let reset_trader = construct_group_three(&mut session);
        assert_eq!(
            (reset_trader.max_life, reset_trader.life),
            (
                AUTHORED_MAX_LIFE.cast_signed(),
                AUTHORED_MAX_LIFE.cast_signed()
            )
        );

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this stages the existing real Level 2 group-3 boundary
        // and proves one Trader maximum-life edit crosses guarded publication,
        // old-session isolation, explicit reopen/reset, the ordinary constructor
        // and action-113 mailbox damage branch. It does not prove organic group
        // progression, Trader targeting/death cleanup, other Traders, placement,
        // save/replay, or Unity UI.
    }

    #[test]
    fn campaign_backend_binds_real_level_two_imported_villager_max_life_before_ordinary_damage_after_reopen_and_reset()
     {
        const PACKAGE_MAX_LIFE: u32 = 1_000;
        const AUTHORED_MAX_LIFE: u32 = 2_000;
        const DAMAGE: i32 = 1_500;

        let root = descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let villager =
            sim_definition::adapter::stable_entity_id("creature.villager.at_121_180").unwrap();
        let package_entity = base.level_definition().unwrap().entity(&villager).unwrap();
        assert_eq!(
            package_entity.creature_health().map(|health| (
                health.default_max_life(),
                health.max_life(),
                health.max_life_access(),
            )),
            Some((
                PACKAGE_MAX_LIFE,
                PACKAGE_MAX_LIFE,
                LevelEntityParameterAccess::SessionReset,
            ))
        );

        let workspace_root = temporary_workspace_path("campaign-villager-max-life");
        let store =
            WorkspaceStore::create(&workspace_root, "campaign-villager-max-life", base.clone())
                .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            95,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let source_slot = match session
            .backend()
            .campaign
            .definition()
            .movable_entity_binding(&villager)
            .unwrap()
        {
            crate::campaign::spec::Mc2EntityRuntimeBinding::InitialSource {
                initial_source_slot,
                kind: LevelEntityKind::Creature(CreatureKind::Villager),
                ..
            } => *initial_source_slot,
            binding => panic!("Villager has the wrong runtime binding: {binding:?}"),
        };
        let villager_state = |session: &GameSession<Mc2CampaignBackend>| {
            session
                .backend()
                .campaign
                .level2_compatibility()
                .ordinary()
                .villagers()
                .iter()
                .find(|state| state.entity_index == source_slot)
                .cloned()
                .unwrap()
        };
        let stage_damage = |session: &mut GameSession<Mc2CampaignBackend>| {
            session
                .backend_mut()
                .campaign
                .level2_compatibility_mut()
                .ordinary_mut()
                .inject_villager_damage(source_slot, DAMAGE, 1)
                .unwrap();
        };

        let change_set = ChangeSetBuilder::new(
            "campaign-villager-max-life",
            &base,
            "raise-one-level-two-villager-max-life",
            RequestedTiming::SessionReset,
        )
        .set_entity_max_life_from(villager, PACKAGE_MAX_LIFE, AUTHORED_MAX_LIFE)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();

        let package_state = villager_state(&session);
        assert_eq!(
            (package_state.max_life, package_state.life),
            (
                PACKAGE_MAX_LIFE.cast_signed(),
                PACKAGE_MAX_LIFE.cast_signed()
            )
        );
        stage_damage(&mut session);
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        let package_after = villager_state(&session);
        assert_eq!(
            (package_after.life, package_after.action),
            (-500, crate::villager::VILLAGER_DEATH_ACTION)
        );
        assert_eq!(session.identity().definition_hash, base.definition_hash());

        let reopened = session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        assert_eq!(
            (
                reopened.identity.generation,
                reopened.identity.definition_revision
            ),
            (2, 2)
        );
        let authored = villager_state(&session);
        assert_eq!(
            (authored.max_life, authored.life),
            (
                AUTHORED_MAX_LIFE.cast_signed(),
                AUTHORED_MAX_LIFE.cast_signed()
            )
        );
        let raw =
            &session.backend().campaign.level2_compatibility().entities()[usize::from(source_slot)];
        assert_eq!((raw.max_life, raw.life), (authored.max_life, authored.life));
        let authored_reset_hash = session.backend().campaign.state_hash();

        stage_damage(&mut session);
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        let authored_after = villager_state(&session);
        assert_eq!(
            (authored_after.life, authored_after.action),
            (500, crate::villager::VILLAGER_HIT_ACTION)
        );

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.backend().campaign.state_hash(), authored_reset_hash);
        let reset = villager_state(&session);
        assert_eq!(reset, authored);
        let raw_reset =
            &session.backend().campaign.level2_compatibility().entities()[usize::from(source_slot)];
        assert_eq!(
            (raw_reset.max_life, raw_reset.life),
            (reset.max_life, reset.life)
        );

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this proves one reset-live Level 2 Villager maximum-life
        // edit crosses guarded publication, old-session isolation, raw/typed
        // frame-zero synchronization, explicit reopen/reset, and the ordinary
        // damage mailbox. It does not prove organic targeting/building entry,
        // death cleanup/population effects, other Villagers, save/replay, or UI.
    }

    #[test]
    fn campaign_backend_moves_real_level_two_imported_villager_before_ordinary_visit_after_reopen()
    {
        let root = descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let villager =
            sim_definition::adapter::stable_entity_id("creature.villager.at_121_180").unwrap();
        let package_tile = TilePosition { x: 121, y: 180 };
        let authored_tile = TilePosition { x: 130, y: 180 };
        let package_entity = base.level_definition().unwrap().entity(&villager).unwrap();
        assert_eq!(
            package_entity.kind(),
            LevelEntityKind::Creature(CreatureKind::Villager)
        );
        assert_eq!(package_entity.position(), package_tile);
        assert_eq!(
            package_entity.position_access(),
            LevelEntityPositionAccess::SessionReset
        );

        let workspace_root = temporary_workspace_path("campaign-imported-villager-placement");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-imported-villager-placement",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            89,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let mut package_occurrences = session
            .backend()
            .campaign
            .level2_compatibility()
            .ordinary()
            .villagers()
            .iter()
            .filter(|state| {
                (state.position.x >> 8, state.position.y >> 8) == (package_tile.x, package_tile.y)
            })
            .map(|state| state.entity_index)
            .collect::<Vec<_>>();
        package_occurrences.sort_unstable();
        assert_eq!(package_occurrences.len(), 2);
        let source_slot = package_occurrences[0];
        let villager_bindings = base
            .level_definition()
            .unwrap()
            .entities()
            .iter()
            .filter(|entity| entity.kind() == LevelEntityKind::Creature(CreatureKind::Villager))
            .map(|entity| {
                let crate::campaign::spec::Mc2EntityRuntimeBinding::InitialSource {
                    initial_source_slot,
                    kind,
                    ..
                } = session
                    .backend()
                    .campaign
                    .definition()
                    .movable_entity_binding(entity.id())
                    .unwrap()
                else {
                    panic!("semantic Villager must bind one reset-live source slot");
                };
                assert_eq!(*kind, LevelEntityKind::Creature(CreatureKind::Villager));
                *initial_source_slot
            })
            .collect::<Vec<_>>();
        assert_eq!(villager_bindings.len(), 29);
        assert_eq!(
            villager_bindings
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len(),
            29,
            "duplicate semantic tiles must still bind distinct source occurrences"
        );
        assert!(matches!(
            session
                .backend()
                .campaign
                .definition()
                .movable_entity_binding(&villager),
            Some(crate::campaign::spec::Mc2EntityRuntimeBinding::InitialSource {
                initial_source_slot,
                kind: LevelEntityKind::Creature(CreatureKind::Villager),
                ..
            }) if *initial_source_slot == source_slot
        ));
        let villager_state = |session: &GameSession<Mc2CampaignBackend>| {
            session
                .backend()
                .campaign
                .level2_compatibility()
                .ordinary()
                .villagers()
                .iter()
                .find(|state| state.entity_index == source_slot)
                .cloned()
                .unwrap()
        };
        let package_state = villager_state(&session);
        assert_eq!(
            (package_state.position.x >> 8, package_state.position.y >> 8),
            (package_tile.x, package_tile.y)
        );

        let change_set = ChangeSetBuilder::new(
            "campaign-imported-villager-placement",
            &base,
            "move-one-level-two-imported-villager",
            RequestedTiming::SessionReset,
        )
        .move_entity_from(villager, package_tile, authored_tile)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();
        assert_eq!(villager_state(&session), package_state);
        assert_eq!(session.identity().definition_hash, base.definition_hash());

        let reopened = session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        assert_eq!(
            (
                reopened.identity.generation,
                reopened.identity.definition_revision
            ),
            (2, 2)
        );
        let authored = villager_state(&session);
        assert_eq!(
            (authored.position.x >> 8, authored.position.y >> 8),
            (authored_tile.x, authored_tile.y)
        );
        assert_eq!(authored.entity_index, source_slot);
        let compatibility = session.backend().campaign.level2_compatibility();
        assert_eq!(
            compatibility.entities()[usize::from(source_slot)].position,
            authored.position
        );
        assert_eq!(
            compatibility.economy().map_chains().tile_of(source_slot),
            u16::from_le_bytes([
                u8::try_from(authored_tile.x).unwrap(),
                u8::try_from(authored_tile.y).unwrap(),
            ])
        );
        let authored_reset_hash = session.backend().campaign.state_hash();

        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        let visited = villager_state(&session);
        assert_eq!(visited.phase, authored.phase.wrapping_add(1));
        assert!(
            (visited.position.x >> 8).abs_diff(authored_tile.x) <= 1,
            "ordinary Villager movement must start from the authored tile: {visited:?}"
        );
        assert!(
            session
                .backend()
                .campaign
                .events()
                .iter()
                .any(|event| matches!(
                    event,
                    Level2DispatchEvent::Villager {
                        source_slot: actual,
                        ..
                    } if *actual == source_slot
                ))
        );

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.backend().campaign.state_hash(), authored_reset_hash);
        let reset = villager_state(&session);
        assert_eq!(reset, authored);
        assert_eq!(
            session.backend().campaign.level2_compatibility().entities()[usize::from(source_slot)]
                .position,
            authored.position
        );

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this proves one reset-live Level 2 Villager crosses
        // guarded publication, old-session isolation, the exact private
        // source-record binding, raw/typed/map synchronization, explicit
        // reopen, one ordinary Villager visit, and reset. It does not prove
        // the other fifty-six placements, organic targeting/building entry,
        // combat/death, population changes, save/replay, or Unity presentation.
    }

    #[test]
    fn campaign_backend_moves_real_level_one_house_before_ordinary_action_52_after_reopen() {
        let root = level_one_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let workspace_root = temporary_workspace_path("campaign-house-placement");
        let store =
            WorkspaceStore::create(&workspace_root, "campaign-house-placement", base.clone())
                .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            90,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let house_bindings = base
            .level_definition()
            .unwrap()
            .entities()
            .iter()
            .filter(|entity| {
                entity.kind() == LevelEntityKind::Building(BuildingKind::House)
                    && entity.position_access() == LevelEntityPositionAccess::SessionReset
            })
            .filter_map(|entity| {
                match session
                    .backend()
                    .campaign
                    .definition()
                    .movable_entity_binding(entity.id())
                    .unwrap()
                {
                    crate::campaign::spec::Mc2EntityRuntimeBinding::InitialSource {
                        initial_source_slot,
                        kind,
                        ..
                    } => {
                        assert_eq!(*kind, LevelEntityKind::Building(BuildingKind::House));
                        Some((entity, *initial_source_slot))
                    }
                    crate::campaign::spec::Mc2EntityRuntimeBinding::AuthoredFeature { .. } => None,
                    crate::campaign::spec::Mc2EntityRuntimeBinding::InitialMultipartFamily {
                        ..
                    } => panic!("semantic House cannot bind a multipart carrier"),
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(house_bindings.len(), 23);
        assert_eq!(
            house_bindings
                .iter()
                .map(|(_, source_slot)| *source_slot)
                .collect::<BTreeSet<_>>()
                .len(),
            house_bindings.len(),
            "duplicate semantic House tiles must bind distinct source occurrences"
        );
        let package_house = session
            .backend()
            .campaign
            .level2_compatibility()
            .ordinary()
            .houses()
            .iter()
            .find(|house| {
                house.active
                    && house.action == 52
                    && house_bindings
                        .iter()
                        .any(|(_, source_slot)| *source_slot == house.entity_index)
            })
            .copied()
            .expect("real Level 1 contains a position-bound ordinary action-52 House");
        let (house_entity, source_slot) = house_bindings
            .into_iter()
            .find(|(_, source_slot)| *source_slot == package_house.entity_index)
            .expect("ordinary action-52 House has one semantic identity");
        let house_id = house_entity.id().clone();
        let package_tile = house_entity.position();
        let authored_tile = TilePosition {
            x: if package_tile.x < u16::from(u8::MAX) {
                package_tile.x + 1
            } else {
                package_tile.x - 1
            },
            y: package_tile.y,
        };
        assert_eq!(source_slot, package_house.entity_index);
        assert_eq!(
            (package_house.position.x >> 8, package_house.position.y >> 8),
            (package_tile.x, package_tile.y)
        );

        let change_set = ChangeSetBuilder::new(
            "campaign-house-placement",
            &base,
            "move-one-level-one-house",
            RequestedTiming::SessionReset,
        )
        .move_entity_from(house_id, package_tile, authored_tile)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();
        assert_eq!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .ordinary()
                .houses()
                .iter()
                .find(|house| house.entity_index == source_slot)
                .copied(),
            Some(package_house)
        );
        assert_eq!(session.identity().definition_hash, base.definition_hash());

        let reopened = session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        assert_eq!(
            (
                reopened.identity.generation,
                reopened.identity.definition_revision
            ),
            (2, 2)
        );
        let authored = session
            .backend()
            .campaign
            .level2_compatibility()
            .ordinary()
            .houses()
            .iter()
            .find(|house| house.entity_index == source_slot)
            .copied()
            .unwrap();
        assert_eq!(
            (authored.position.x >> 8, authored.position.y >> 8),
            (authored_tile.x, authored_tile.y)
        );
        let compatibility = session.backend().campaign.level2_compatibility();
        assert_eq!(
            compatibility.entities()[usize::from(source_slot)].position,
            authored.position
        );
        assert_eq!(
            compatibility.economy().map_chains().tile_of(source_slot),
            u16::from_le_bytes([
                u8::try_from(authored_tile.x).unwrap(),
                u8::try_from(authored_tile.y).unwrap(),
            ])
        );
        let authored_reset_hash = session.backend().campaign.state_hash();

        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        let visited = session
            .backend()
            .campaign
            .level2_compatibility()
            .ordinary()
            .houses()
            .iter()
            .find(|house| house.entity_index == source_slot)
            .copied()
            .expect("ordinary House remains live after action 52");
        assert_eq!(visited.phase, authored.phase.wrapping_add(1));
        assert_eq!(
            (visited.position.x >> 8, visited.position.y >> 8),
            (authored_tile.x, authored_tile.y)
        );
        assert!(
            session
                .backend()
                .campaign
                .events()
                .iter()
                .any(|event| matches!(
                    event,
                    Level2DispatchEvent::House {
                        source_slot: actual,
                        ..
                    } if *actual == source_slot
                ))
        );

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.backend().campaign.state_hash(), authored_reset_hash);
        assert_eq!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .ordinary()
                .houses()
                .iter()
                .find(|house| house.entity_index == source_slot)
                .copied(),
            Some(authored)
        );
        assert_eq!(
            session.backend().campaign.level2_compatibility().entities()[usize::from(source_slot)]
                .position,
            authored.position
        );

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this proves one reset-live Level 1 House crosses guarded
        // publication, old-session isolation, the exact private occurrence
        // binding, raw/typed/map synchronization, explicit reopen, one real
        // action-52 visit, and reset. It does not prove the other eighty-four
        // writable placements, construction parameters, ownership, population,
        // damage/destruction, save/replay, or Unity presentation.
    }

    #[test]
    fn campaign_backend_moves_real_grouped_level_one_house_through_construction_after_reopen() {
        const HOUSE_RECORD: u16 = 107;
        const GROUP: u16 = 13;

        fn trigger_group_thirteen(session: &mut GameSession<Mc2CampaignBackend>) -> u16 {
            let trigger_id =
                sim_definition::adapter::stable_entity_id("switch.area_entry_trigger.at_165_211")
                    .unwrap();
            let trigger_slot = match session
                .backend()
                .campaign
                .definition()
                .movable_entity_binding(&trigger_id)
                .unwrap()
            {
                crate::campaign::spec::Mc2EntityRuntimeBinding::InitialSource {
                    initial_source_slot,
                    kind: LevelEntityKind::Switch(SwitchKind::AreaEntryTrigger),
                    ..
                } => *initial_source_slot,
                binding => panic!("group-13 trigger has the wrong runtime binding: {binding:?}"),
            };
            let (player_slot, trigger_position) = {
                let compatibility = session.backend().campaign.level2_compatibility();
                (
                    compatibility.state.entity_index,
                    compatibility.entities()[usize::from(trigger_slot)].position,
                )
            };
            session
                .backend_mut()
                .campaign
                .level2_compatibility_mut()
                .set_level2_entity_position(player_slot, trigger_position)
                .unwrap();

            for _ in 0..8 {
                session
                    .step(guard(session), GameCommand::default())
                    .unwrap();
                if session.backend().campaign.events().iter().any(|event| {
                    matches!(
                        event,
                        Level2DispatchEvent::AuthoredGroupActivated {
                            source_slot,
                            group: GROUP,
                            authored_indices,
                            ..
                        } if *source_slot == trigger_slot
                            && authored_indices.contains(&HOUSE_RECORD)
                    )
                }) {
                    break;
                }
            }
            let compatibility = session.backend().campaign.level2_compatibility();
            assert!(compatibility.dispatch_events().iter().any(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::AuthoredGroupActivated {
                        source_slot,
                        group: GROUP,
                        authored_indices,
                        ..
                    } if *source_slot == trigger_slot
                        && authored_indices.contains(&HOUSE_RECORD)
                )
            }));
            assert!(
                compatibility
                    .dispatch_events()
                    .iter()
                    .all(|event| { !matches!(event, Level2DispatchEvent::Error { .. }) })
            );
            compatibility
                .economy()
                .authored_house_source_slot(HOUSE_RECORD)
                .expect("the real group-13 producer constructs House record 107")
        }

        fn expected_house_position(
            session: &GameSession<Mc2CampaignBackend>,
            tile: TilePosition,
        ) -> crate::Position {
            let compatibility = session.backend().campaign.level2_compatibility();
            crate::house::authored_level_one_house_position(
                crate::Position {
                    x: tile.x << 8,
                    y: tile.y << 8,
                    z: 0,
                },
                Some(compatibility.canonical_terrain()),
            )
        }

        fn constructed_house(
            session: &GameSession<Mc2CampaignBackend>,
            source_slot: u16,
        ) -> crate::house::HouseState {
            session
                .backend()
                .campaign
                .level2_compatibility()
                .ordinary()
                .houses()
                .iter()
                .find(|house| house.active && house.entity_index == source_slot)
                .copied()
                .expect("the authored House remains in the ordinary shared family")
        }

        let root = level_one_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let house_id =
            sim_definition::adapter::stable_entity_id("building.house.at_156_215").unwrap();
        let house = base.level_definition().unwrap().entity(&house_id).unwrap();
        let package_tile = TilePosition { x: 156, y: 215 };
        let authored_tile = TilePosition { x: 164, y: 216 };
        assert_eq!(house.kind(), LevelEntityKind::Building(BuildingKind::House));
        assert_eq!(house.position(), package_tile);
        assert_eq!(
            house.position_access(),
            LevelEntityPositionAccess::SessionReset
        );

        let workspace_root = temporary_workspace_path("campaign-grouped-house-placement");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-grouped-house-placement",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            94,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        assert!(matches!(
            session
                .backend()
                .campaign
                .definition()
                .movable_entity_binding(&house_id),
            Some(
                crate::campaign::spec::Mc2EntityRuntimeBinding::AuthoredFeature {
                    authored_record: HOUSE_RECORD,
                    kind: LevelEntityKind::Building(BuildingKind::House),
                }
            )
        ));

        let change_set = ChangeSetBuilder::new(
            "campaign-grouped-house-placement",
            &base,
            "move-level-one-grouped-house",
            RequestedTiming::SessionReset,
        )
        .move_entity_from(house_id, package_tile, authored_tile)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();

        let package_slot = trigger_group_thirteen(&mut session);
        let package_house = constructed_house(&session, package_slot);
        assert_eq!(
            package_house.position,
            expected_house_position(&session, package_tile),
            "publication must not mutate the already-open package generation"
        );
        assert_eq!(session.identity().definition_hash, base.definition_hash());

        let reopened = session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        assert_eq!(reopened.identity.generation, 2);
        assert_eq!(reopened.identity.definition_revision, 2);
        let compatibility = session.backend().campaign.level2_compatibility();
        assert_eq!(
            compatibility
                .mission()
                .authored_features()
                .iter()
                .find(|feature| feature.index == HOUSE_RECORD)
                .map(|feature| (feature.tile_x, feature.tile_y)),
            Some((authored_tile.x, authored_tile.y))
        );
        let authored_reset_hash = session.backend().campaign.state_hash();

        let authored_slot = trigger_group_thirteen(&mut session);
        let authored = constructed_house(&session, authored_slot);
        let authored_position = expected_house_position(&session, authored_tile);
        assert_eq!(authored.position, authored_position);
        assert_ne!(
            authored.position,
            expected_house_position(&session, package_tile)
        );
        assert_eq!(authored.action, crate::house::HOUSE_TERRAIN_TO_LIVE_ACTION);
        assert_eq!(
            authored.building_index,
            crate::house::AUTHORED_HOUSE_BUILDING_INDEX
        );
        assert!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .economy()
                .map_chains()
                .is_linked(authored_slot)
        );
        let starting_life = authored.life;
        assert!((1..=30).contains(&starting_life));
        let mut saw_construction_pass = session.backend().campaign.events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::House {
                    source_slot,
                    events,
                    ..
                } if *source_slot == authored_slot
                    && events.iter().any(|event| matches!(
                        event,
                        crate::house::HouseStepEvent::ConstructionPass { .. }
                    ))
            )
        });
        let mut visits = 0_usize;
        while constructed_house(&session, authored_slot).action
            == crate::house::HOUSE_TERRAIN_TO_LIVE_ACTION
        {
            assert!(visits < usize::try_from(starting_life).unwrap());
            session
                .step(guard(&session), GameCommand::default())
                .unwrap();
            visits += 1;
            saw_construction_pass |= session.backend().campaign.events().iter().any(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::House {
                        source_slot,
                        events,
                        ..
                    } if *source_slot == authored_slot
                        && events.iter().any(|event| matches!(
                            event,
                            crate::house::HouseStepEvent::ConstructionPass { .. }
                        ))
                )
            });
        }
        let live = constructed_house(&session, authored_slot);
        assert!(saw_construction_pass);
        assert_eq!(visits, usize::try_from(starting_life).unwrap());
        assert_eq!(live.action, crate::house::HOUSE_LIVE_ACTION);
        assert_eq!(live.life, 20_000);
        assert_eq!(
            (live.position.x, live.position.y),
            (authored_position.x, authored_position.y)
        );
        let terrain = session
            .backend()
            .campaign
            .level2_compatibility()
            .canonical_terrain();
        assert_ne!(live.position.z, authored_position.z);
        assert_eq!(
            live.position.z,
            crate::FlightWorld::ground_height(terrain, live.position)
        );
        assert!(session.backend().campaign.events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::House {
                    source_slot,
                    events,
                    ..
                } if *source_slot == authored_slot
                    && events.iter().any(|event| matches!(
                        event,
                        crate::house::HouseStepEvent::ConstructionCompleted { .. }
                    ))
            )
        }));
        assert!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .economy()
                .map_chains()
                .is_linked(authored_slot)
        );

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.backend().campaign.state_hash(), authored_reset_hash);
        let reset = session.backend().campaign.level2_compatibility();
        assert_eq!(
            reset.economy().authored_house_source_slot(HOUSE_RECORD),
            None
        );
        assert!(
            reset
                .ordinary()
                .houses()
                .iter()
                .all(|house| { house.entity_index != authored_slot || !house.active })
        );
        assert_eq!(
            reset
                .mission()
                .authored_features()
                .iter()
                .find(|feature| feature.index == HOUSE_RECORD)
                .map(|feature| (feature.tile_x, feature.tile_y)),
            Some((authored_tile.x, authored_tile.y))
        );
        let reset_slot = trigger_group_thirteen(&mut session);
        assert_eq!(
            constructed_house(&session, reset_slot).position,
            authored_position
        );

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this proves one positive-group Level 1 House placement
        // crosses guarded publication, old-session isolation, the real
        // group-13 area-entry producer, source-correct construction, the
        // ordinary action-51 countdown/completion, map retention, and reset.
        // It does not prove the other thirty-three grouped placements,
        // construction parameter authoring, ownership, residents, damage or
        // destruction, the read-only group--1 Houses, Level 2/3 grouped Houses,
        // save/replay, or Unity presentation.
    }

    #[test]
    fn campaign_backend_binds_real_grouped_house_life_mana_scalar_before_construction_damage_after_reopen_and_reset()
     {
        const HOUSE_RECORD: u16 = 107;
        const GROUP: u16 = 13;

        fn trigger_group_thirteen(session: &mut GameSession<Mc2CampaignBackend>) -> u16 {
            let trigger_id =
                sim_definition::adapter::stable_entity_id("switch.area_entry_trigger.at_165_211")
                    .unwrap();
            let trigger_slot = match session
                .backend()
                .campaign
                .definition()
                .movable_entity_binding(&trigger_id)
                .unwrap()
            {
                crate::campaign::spec::Mc2EntityRuntimeBinding::InitialSource {
                    initial_source_slot,
                    kind: LevelEntityKind::Switch(SwitchKind::AreaEntryTrigger),
                    ..
                } => *initial_source_slot,
                binding => panic!("group-13 trigger has the wrong runtime binding: {binding:?}"),
            };
            let (player_slot, trigger_position) = {
                let compatibility = session.backend().campaign.level2_compatibility();
                (
                    compatibility.state.entity_index,
                    compatibility.entities()[usize::from(trigger_slot)].position,
                )
            };
            session
                .backend_mut()
                .campaign
                .level2_compatibility_mut()
                .set_level2_entity_position(player_slot, trigger_position)
                .unwrap();
            for _ in 0..8 {
                session
                    .step(guard(session), GameCommand::default())
                    .unwrap();
                if session.backend().campaign.events().iter().any(|event| {
                    matches!(
                        event,
                        Level2DispatchEvent::AuthoredGroupActivated {
                            source_slot,
                            group: GROUP,
                            authored_indices,
                            ..
                        } if *source_slot == trigger_slot
                            && authored_indices.contains(&HOUSE_RECORD)
                    )
                }) {
                    break;
                }
            }
            session
                .backend()
                .campaign
                .level2_compatibility()
                .economy()
                .authored_house_source_slot(HOUSE_RECORD)
                .expect("the real group-13 producer constructs House record 107")
        }

        fn house(
            session: &GameSession<Mc2CampaignBackend>,
            source_slot: u16,
        ) -> crate::house::HouseState {
            session
                .backend()
                .campaign
                .level2_compatibility()
                .ordinary()
                .houses()
                .iter()
                .find(|house| house.active && house.entity_index == source_slot)
                .copied()
                .expect("grouped House remains in the ordinary typed family")
        }

        fn finish_construction(
            session: &mut GameSession<Mc2CampaignBackend>,
            source_slot: u16,
        ) -> (crate::house::HouseState, usize) {
            let initial_life = house(session, source_slot).life;
            let mut visits = usize::try_from(30_i32.wrapping_sub(initial_life)).unwrap();
            while house(session, source_slot).action == crate::house::HOUSE_TERRAIN_TO_LIVE_ACTION {
                assert!(visits < 31, "grouped House construction did not complete");
                session
                    .step(guard(session), GameCommand::default())
                    .unwrap();
                visits += 1;
            }
            (house(session, source_slot), visits)
        }

        let root = level_one_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let house_id =
            sim_definition::adapter::stable_entity_id("building.house.at_156_215").unwrap();
        let package_house = base.level_definition().unwrap().entity(&house_id).unwrap();
        assert_eq!(
            package_house
                .house_construction()
                .expect("grouped House exposes its construction scalar")
                .life_mana_scalar(),
            20
        );
        assert_eq!(
            base.level_definition()
                .unwrap()
                .entities()
                .iter()
                .filter(|entity| entity.house_construction().is_some())
                .count(),
            34
        );
        assert!(package_house.creature_health().is_none());
        assert!(package_house.absence_countdown().is_none());

        let workspace_root = temporary_workspace_path("campaign-grouped-house-scalar");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-grouped-house-scalar",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            95,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();

        let change_set = ChangeSetBuilder::new(
            "campaign-grouped-house-scalar",
            &base,
            "set-level-one-grouped-house-scalar",
            RequestedTiming::SessionReset,
        )
        .set_house_life_mana_scalar_from(house_id.clone(), 20, 40)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();
        assert_eq!(
            published
                .level_definition()
                .unwrap()
                .entity(&house_id)
                .unwrap()
                .house_construction()
                .unwrap()
                .life_mana_scalar(),
            40
        );

        let fake = ChangeSetBuilder::new(
            "campaign-grouped-house-scalar",
            &published,
            "reject-house-max-life-countdown-fake",
            RequestedTiming::SessionReset,
        )
        .set_entity_max_life(house_id.clone(), 40)
        .set_mechanic_countdown(house_id.clone(), 40)
        .build()
        .unwrap();
        let fake_validation = store.validate(&catalog, fake).unwrap();
        assert!(!fake_validation.accepted);

        let package_slot = trigger_group_thirteen(&mut session);
        let (package_live, package_visits) = finish_construction(&mut session, package_slot);
        assert_eq!(package_visits, 30);
        assert_eq!(package_live.parameter.life_mana_scalar, 20);
        assert_eq!(package_live.life, 20_000);
        session
            .backend_mut()
            .campaign
            .level2_compatibility_mut()
            .ordinary_mut()
            .inject_house_damage(package_slot, 25_000, 1)
            .unwrap();
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        assert_eq!(
            house(&session, package_slot).action,
            crate::house::HOUSE_REMOVE_ACTION
        );

        session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        let authored_reset_hash = session.backend().campaign.state_hash();
        let authored_slot = trigger_group_thirteen(&mut session);
        let (authored_live, authored_visits) = finish_construction(&mut session, authored_slot);
        assert_eq!(authored_visits, 30);
        assert_eq!(authored_live.parameter.life_mana_scalar, 40);
        assert_eq!(authored_live.life, 40_000);
        session
            .backend_mut()
            .campaign
            .level2_compatibility_mut()
            .ordinary_mut()
            .inject_house_damage(authored_slot, 25_000, 1)
            .unwrap();
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        let surviving = house(&session, authored_slot);
        assert_eq!(surviving.action, crate::house::HOUSE_LIVE_ACTION);
        assert_eq!(surviving.life, 15_000);

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.backend().campaign.state_hash(), authored_reset_hash);
        let reset_slot = trigger_group_thirteen(&mut session);
        let (reset_live, reset_visits) = finish_construction(&mut session, reset_slot);
        assert_eq!(reset_visits, 30);
        assert_eq!(reset_live.parameter.life_mana_scalar, 40);
        assert_eq!(reset_live.life, 40_000);

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: proves one source-derived construction parameter across guarded publication, old-session isolation, real group production, action-51 completion, action-52 damage discrimination and reset for the 34 complete-lifecycle Level-1 grouped Houses. It does not author construction countdown, footprint, building selector, ownership, residents, population, the reset-live/unbound Houses, Level 2/3 Houses, save/replay, or Unity UI.
    }

    #[test]
    fn campaign_backend_moves_real_grouped_firefly_before_ordinary_creature_visit_after_reopen() {
        const FIREFLY_RECORD: u16 = 134;
        const GROUP_SIX_ROWS: &[u16] = &[108, 134, 135, 154, 189, 193];

        let root = level_one_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let firefly =
            sim_definition::adapter::stable_entity_id("creature.firefly.at_213_125").unwrap();
        let package_tile = TilePosition { x: 213, y: 125 };
        let authored_tile = TilePosition { x: 220, y: 126 };
        let package_entity = base.level_definition().unwrap().entity(&firefly).unwrap();
        assert_eq!(
            package_entity.kind(),
            LevelEntityKind::Creature(CreatureKind::Firefly)
        );
        assert_eq!(package_entity.position(), package_tile);
        assert_eq!(
            package_entity.position_access(),
            LevelEntityPositionAccess::SessionReset
        );

        let workspace_root = temporary_workspace_path("campaign-grouped-firefly-placement");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-grouped-firefly-placement",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            77,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let change_set = ChangeSetBuilder::new(
            "campaign-grouped-firefly-placement",
            &base,
            "move-level-one-grouped-firefly",
            RequestedTiming::SessionReset,
        )
        .move_entity_from(firefly.clone(), package_tile, authored_tile)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();

        let construct_group_six = |session: &mut GameSession<Mc2CampaignBackend>| {
            let compatibility = session.backend_mut().campaign.level2_compatibility_mut();
            compatibility
                .activate_authored_group_prerequisite_for_test(6)
                .unwrap();
            assert!(compatibility.dispatch_events().iter().any(|event| matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated {
                    group: 6,
                    authored_indices,
                    ..
                } if authored_indices.as_ref() == GROUP_SIX_ROWS
            )));
            compatibility
                .economy()
                .authored_class5()
                .iter()
                .find(|source| source.authored_index == FIREFLY_RECORD)
                .copied()
                .unwrap()
        };

        let package_firefly = construct_group_six(&mut session);
        assert_eq!(
            (
                package_firefly.position.x >> 8,
                package_firefly.position.y >> 8
            ),
            (package_tile.x, package_tile.y),
            "publication must not mutate an already-open session"
        );
        assert_eq!(session.identity().definition_hash, base.definition_hash());

        let reopened = session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        assert_eq!(reopened.identity.generation, 2);
        assert_eq!(reopened.identity.definition_revision, 2);
        let compatibility = session.backend().campaign.level2_compatibility();
        assert_eq!(
            compatibility
                .mission()
                .authored_features()
                .iter()
                .find(|feature| feature.index == FIREFLY_RECORD)
                .map(|feature| (feature.tile_x, feature.tile_y)),
            Some((authored_tile.x, authored_tile.y))
        );
        let authored_reset_hash = session.backend().campaign.state_hash();

        let authored_firefly = construct_group_six(&mut session);
        assert_eq!(
            (
                authored_firefly.position.x >> 8,
                authored_firefly.position.y >> 8
            ),
            (authored_tile.x, authored_tile.y)
        );
        assert_ne!(
            (
                authored_firefly.position.x >> 8,
                authored_firefly.position.y >> 8
            ),
            (package_tile.x, package_tile.y)
        );
        let source_slot = authored_firefly.source_entity_index;
        assert!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .economy()
                .map_chains()
                .is_linked(source_slot)
        );

        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        let compatibility = session.backend().campaign.level2_compatibility();
        let visited = compatibility
            .economy()
            .authored_class5()
            .iter()
            .find(|source| source.source_entity_index == source_slot)
            .copied()
            .unwrap();
        assert_eq!(
            visited.source_byte_3e,
            authored_firefly.source_byte_3e.wrapping_add(1)
        );
        assert_ne!(visited.position, authored_firefly.position);
        assert!(visited.active && visited.drawing_enabled);
        assert!(compatibility.economy().map_chains().is_linked(source_slot));
        assert!(
            session
                .backend()
                .campaign
                .events()
                .iter()
                .any(|event| matches!(
                    event,
                    Level2DispatchEvent::DynamicArena {
                        source_slot: actual,
                        kind: crate::native_world::Level2DynamicArenaKind::AuthoredClass5,
                        ..
                    } if *actual == source_slot
                ))
        );
        assert!(
            session
                .backend()
                .campaign
                .events()
                .iter()
                .all(|event| !matches!(
                    event,
                    Level2DispatchEvent::Error {
                        source_slot: actual,
                        ..
                    } if *actual == source_slot
                ))
        );

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.backend().campaign.state_hash(), authored_reset_hash);
        let reset = session.backend().campaign.level2_compatibility();
        assert!(reset.economy().authored_class5().is_empty());
        assert_eq!(
            reset
                .mission()
                .authored_features()
                .iter()
                .find(|feature| feature.index == FIREFLY_RECORD)
                .map(|feature| (feature.tile_x, feature.tile_y)),
            Some((authored_tile.x, authored_tile.y))
        );
        let reset_firefly = construct_group_six(&mut session);
        assert_eq!(
            (reset_firefly.position.x >> 8, reset_firefly.position.y >> 8),
            (authored_tile.x, authored_tile.y)
        );

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this stages the already-proven Level 1 group-6 boundary to
        // prove that one grouped complete-lifecycle creature placement crosses
        // guarded publication, explicit reopen, ordinary Firefly movement/map
        // consumption, and reset. It does not re-prove creature combat/death,
        // each family's distinct callback, organic prior objectives, reset-live
        // Skeleton placement, unsupported creature families, save/replay, or
        // Unity presentation.
    }

    #[test]
    fn campaign_backend_binds_real_firefly_max_life_before_damage_after_reopen_and_reset() {
        const FIREFLY_RECORD: u16 = 134;
        const GROUP_SIX_ROWS: &[u16] = &[108, 134, 135, 154, 189, 193];
        const PACKAGE_MAX_LIFE: u32 = 600;
        const AUTHORED_MAX_LIFE: u32 = 1_200;

        let root = level_one_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let firefly =
            sim_definition::adapter::stable_entity_id("creature.firefly.at_213_125").unwrap();
        let target = base.level_definition().unwrap().entity(&firefly).unwrap();
        assert_eq!(
            target.creature_health().map(|health| (
                health.default_max_life(),
                health.max_life(),
                health.max_life_access(),
            )),
            Some((
                PACKAGE_MAX_LIFE,
                PACKAGE_MAX_LIFE,
                LevelEntityParameterAccess::SessionReset,
            ))
        );
        assert_eq!(
            base.level_definition()
                .unwrap()
                .entities()
                .iter()
                .filter(|entity| {
                    entity.creature_health().is_some()
                        && entity.kind()
                            == sim_definition::level_definition::LevelEntityKind::Creature(
                                CreatureKind::Firefly,
                            )
                })
                .count(),
            6,
            "all package-present Fireflies share this bound constructor parameter"
        );
        assert!(base.required_capabilities().iter().any(|requirement| {
            requirement.id.as_str() == FIREFLY_MAX_LIFE_CAPABILITY_ID
                && requirement.exact_version == 1
        }));

        let workspace_root = temporary_workspace_path("campaign-firefly-max-life");
        let store =
            WorkspaceStore::create(&workspace_root, "campaign-firefly-max-life", base.clone())
                .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            78,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let change_set = ChangeSetBuilder::new(
            "campaign-firefly-max-life",
            &base,
            "raise-one-level-one-firefly-max-life",
            RequestedTiming::SessionReset,
        )
        .set_entity_max_life_from(firefly.clone(), PACKAGE_MAX_LIFE, AUTHORED_MAX_LIFE)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();
        assert_eq!(
            published
                .level_definition()
                .unwrap()
                .entity(&firefly)
                .and_then(|entity| entity.creature_health())
                .map(|health| (health.default_max_life(), health.max_life())),
            Some((PACKAGE_MAX_LIFE, AUTHORED_MAX_LIFE))
        );

        let construct_group_six = |session: &mut GameSession<Mc2CampaignBackend>| {
            let compatibility = session.backend_mut().campaign.level2_compatibility_mut();
            compatibility
                .activate_authored_group_prerequisite_for_test(6)
                .unwrap();
            assert!(compatibility.dispatch_events().iter().any(|event| matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated {
                    group: 6,
                    authored_indices,
                    ..
                } if authored_indices.as_ref() == GROUP_SIX_ROWS
            )));
            compatibility
                .economy()
                .authored_class5()
                .iter()
                .find(|source| source.authored_index == FIREFLY_RECORD)
                .copied()
                .unwrap()
        };
        let apply_two_impacts = |session: &mut GameSession<Mc2CampaignBackend>, source_slot| {
            let (position, attacker) = {
                let compatibility = session.backend().campaign.level2_compatibility();
                let source = compatibility
                    .economy()
                    .authored_class5()
                    .iter()
                    .find(|source| source.source_entity_index == source_slot)
                    .unwrap();
                (source.position, compatibility.state.entity_index)
            };
            for _ in 0..2 {
                session
                    .backend_mut()
                    .campaign
                    .level2_compatibility_mut()
                    .apply_fireball_impact_to_firefly_for_test(position, attacker, 500);
            }
            session
                .step(guard(session), GameCommand::default())
                .unwrap();
            session
                .backend()
                .campaign
                .level2_compatibility()
                .economy()
                .authored_class5()
                .iter()
                .find(|source| source.source_entity_index == source_slot)
                .copied()
                .unwrap()
        };

        let package_firefly = construct_group_six(&mut session);
        assert_eq!(
            (package_firefly.max_life, package_firefly.life),
            (
                PACKAGE_MAX_LIFE.cast_signed(),
                PACKAGE_MAX_LIFE.cast_signed()
            ),
            "publication must not mutate the already-open package authority"
        );
        let package_after_damage =
            apply_two_impacts(&mut session, package_firefly.source_entity_index);
        assert_eq!(package_after_damage.life, -400);
        assert_eq!(package_after_damage.action, 156);

        let reopened = session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        assert_eq!(
            (
                reopened.identity.generation,
                reopened.identity.definition_revision
            ),
            (2, 2)
        );
        let authored_frame_zero_hash = session.backend().campaign.state_hash();
        let authored_firefly = construct_group_six(&mut session);
        assert_eq!(
            (authored_firefly.max_life, authored_firefly.life),
            (
                AUTHORED_MAX_LIFE.cast_signed(),
                AUTHORED_MAX_LIFE.cast_signed(),
            )
        );
        let authored_after_damage =
            apply_two_impacts(&mut session, authored_firefly.source_entity_index);
        assert_eq!(authored_after_damage.life, 200);
        assert!(authored_after_damage.active && authored_after_damage.drawing_enabled);
        assert!(!matches!(authored_after_damage.action, 156 | 157));

        session.reset(guard(&session)).unwrap();
        assert_eq!(
            session.backend().campaign.state_hash(),
            authored_frame_zero_hash
        );
        let reset_firefly = construct_group_six(&mut session);
        assert_eq!(
            (reset_firefly.max_life, reset_firefly.life),
            (
                AUTHORED_MAX_LIFE.cast_signed(),
                AUTHORED_MAX_LIFE.cast_signed(),
            )
        );

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this stages the already-proven Level 1 group-6 producer
        // and two source-sized Fireball impacts. It proves guarded per-instance
        // Firefly maximum-life publication, old-session isolation, explicit
        // reopen/reset construction, and the ordinary damage/pre-kill branch.
        // It does not author the other five Fireflies in this transaction,
        // other creature families or parameters, organic objective progression,
        // projectile travel/impact production, save/replay, or Unity UI.
    }

    #[test]
    fn campaign_backend_binds_real_archer_max_life_before_mailbox_damage_after_reopen_and_reset() {
        const ARCHER_RECORD: u16 = 101;
        const GROUP_THREE_ROWS: &[u16] = &[101, 102, 103, 106, 109, 153];
        const PACKAGE_MAX_LIFE: u32 = 1_000;
        const AUTHORED_MAX_LIFE: u32 = 2_000;
        const DAMAGE: i32 = 1_100;

        let root = level_one_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let archer =
            sim_definition::adapter::stable_entity_id("creature.archer.at_202_209").unwrap();
        let target = base.level_definition().unwrap().entity(&archer).unwrap();
        assert_eq!(
            target.creature_health().map(|health| (
                health.default_max_life(),
                health.max_life(),
                health.max_life_access(),
            )),
            Some((
                PACKAGE_MAX_LIFE,
                PACKAGE_MAX_LIFE,
                LevelEntityParameterAccess::SessionReset,
            ))
        );
        assert!(base.required_capabilities().iter().any(|requirement| {
            requirement.id.as_str() == ARCHER_MAX_LIFE_CAPABILITY_ID
                && requirement.exact_version == 1
        }));

        let workspace_root = temporary_workspace_path("campaign-archer-max-life");
        let store =
            WorkspaceStore::create(&workspace_root, "campaign-archer-max-life", base.clone())
                .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            79,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let change_set = ChangeSetBuilder::new(
            "campaign-archer-max-life",
            &base,
            "raise-one-level-one-archer-max-life",
            RequestedTiming::SessionReset,
        )
        .set_entity_max_life_from(archer.clone(), PACKAGE_MAX_LIFE, AUTHORED_MAX_LIFE)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();

        let construct_group_three = |session: &mut GameSession<Mc2CampaignBackend>| {
            let compatibility = session.backend_mut().campaign.level2_compatibility_mut();
            compatibility
                .activate_authored_group_prerequisite_for_test(3)
                .unwrap();
            assert!(compatibility.dispatch_events().iter().any(|event| matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated {
                    group: 3,
                    authored_indices,
                    ..
                } if authored_indices.as_ref() == GROUP_THREE_ROWS
            )));
            compatibility
                .economy()
                .authored_class5()
                .iter()
                .find(|source| source.authored_index == ARCHER_RECORD)
                .copied()
                .unwrap()
        };
        let apply_mailbox_damage = |session: &mut GameSession<Mc2CampaignBackend>, source_slot| {
            let attacker = session
                .backend()
                .campaign
                .level2_compatibility()
                .state
                .entity_index;
            assert!(
                session
                    .backend_mut()
                    .campaign
                    .level2_compatibility_mut()
                    .economy_mut()
                    .stage_archer_mailbox_damage_for_test(source_slot, attacker, DAMAGE)
            );
            session
                .step(guard(session), GameCommand::default())
                .unwrap();
            session
                .backend()
                .campaign
                .level2_compatibility()
                .economy()
                .authored_class5()
                .iter()
                .find(|source| source.source_entity_index == source_slot)
                .copied()
                .unwrap()
        };

        let package_archer = construct_group_three(&mut session);
        assert_eq!(
            (package_archer.max_life, package_archer.life),
            (
                PACKAGE_MAX_LIFE.cast_signed(),
                PACKAGE_MAX_LIFE.cast_signed()
            ),
            "publication must not mutate the already-open package authority"
        );
        let package_after_damage =
            apply_mailbox_damage(&mut session, package_archer.source_entity_index);
        assert_eq!(package_after_damage.life, -100);
        assert_eq!(
            package_after_damage.action,
            crate::native_world::level2_class5::ARCHER_ACTION_HIT
        );

        let reopened = session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        assert_eq!(
            (
                reopened.identity.generation,
                reopened.identity.definition_revision
            ),
            (2, 2)
        );
        let authored_frame_zero_hash = session.backend().campaign.state_hash();
        let authored_archer = construct_group_three(&mut session);
        assert_eq!(
            (authored_archer.max_life, authored_archer.life),
            (
                AUTHORED_MAX_LIFE.cast_signed(),
                AUTHORED_MAX_LIFE.cast_signed()
            )
        );
        let authored_after_damage =
            apply_mailbox_damage(&mut session, authored_archer.source_entity_index);
        assert_eq!(authored_after_damage.life, 900);
        assert!(authored_after_damage.active && authored_after_damage.drawing_enabled);
        assert_ne!(
            authored_after_damage.action,
            crate::native_world::level2_class5::ARCHER_ACTION_HIT
        );

        session.reset(guard(&session)).unwrap();
        assert_eq!(
            session.backend().campaign.state_hash(),
            authored_frame_zero_hash
        );
        let reset_archer = construct_group_three(&mut session);
        assert_eq!(
            (reset_archer.max_life, reset_archer.life),
            (
                AUTHORED_MAX_LIFE.cast_signed(),
                AUTHORED_MAX_LIFE.cast_signed()
            )
        );

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this stages the already-proven Level 1 group-3 producer
        // and one source mailbox hit. It proves guarded per-instance Archer
        // maximum-life publication, old-session isolation, explicit
        // reopen/reset construction, and the ordinary damage/hit branch. It
        // does not author the other package Archers in this transaction,
        // Archer targeting/children or organic combat, other creature
        // families or parameters, save/replay, or Unity UI.
    }

    #[test]
    fn campaign_packages_bind_every_first_three_level_archer_health_descriptor() {
        let catalog = default_catalog().unwrap();
        for (root, expected_archers) in [
            (level_one_descriptor_fixture_path(), 4_usize),
            (descriptor_fixture_path(), 4),
            (level_three_descriptor_fixture_path(), 18),
        ] {
            let definition = load_mc2_campaign(&root, &catalog).unwrap();
            let archers = definition
                .level_definition()
                .unwrap()
                .entities()
                .iter()
                .filter(|entity| {
                    entity.kind()
                        == sim_definition::level_definition::LevelEntityKind::Creature(
                            CreatureKind::Archer,
                        )
                })
                .collect::<Vec<_>>();
            assert_eq!(archers.len(), expected_archers);
            assert!(archers.iter().all(|entity| {
                entity.creature_health().is_some_and(|health| {
                    health.default_max_life() == 1_000
                        && health.max_life() == 1_000
                        && health.max_life_access() == LevelEntityParameterAccess::SessionReset
                })
            }));
            assert!(
                definition
                    .required_capabilities()
                    .iter()
                    .any(|requirement| {
                        requirement.id.as_str() == ARCHER_MAX_LIFE_CAPABILITY_ID
                            && requirement.exact_version == 1
                    })
            );
        }

        // No-Claim: this is the closed first-three-package Archer descriptor
        // census. The separate causal gate owns one real constructor and
        // damage visit; this census does not claim all Archer instances were
        // constructed, damaged, killed, or observed through a client.
    }

    #[test]
    fn campaign_level_two_group_twenty_constructs_authored_archer_health_after_reopen_and_reset() {
        const ARCHER_RECORD: u16 = 232;
        const PACKAGE_MAX_LIFE: u32 = 1_000;
        const AUTHORED_MAX_LIFE: u32 = 2_000;

        let root = descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let archer =
            sim_definition::adapter::stable_entity_id("creature.archer.at_150_144").unwrap();
        let workspace_root = temporary_workspace_path("campaign-level-two-archer-max-life");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-level-two-archer-max-life",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            80,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let change_set = ChangeSetBuilder::new(
            "campaign-level-two-archer-max-life",
            &base,
            "raise-one-level-two-archer-max-life",
            RequestedTiming::SessionReset,
        )
        .set_entity_max_life_from(archer, PACKAGE_MAX_LIFE, AUTHORED_MAX_LIFE)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();

        let construct_group_twenty = |session: &mut GameSession<Mc2CampaignBackend>| {
            let compatibility = session.backend_mut().campaign.level2_compatibility_mut();
            compatibility
                .activate_authored_group_prerequisite_for_test(20)
                .unwrap();
            assert!(compatibility.dispatch_events().iter().any(|event| matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated { group: 20, .. }
            )));
            compatibility
                .economy()
                .authored_class5()
                .iter()
                .find(|source| source.authored_index == ARCHER_RECORD)
                .copied()
                .unwrap()
        };

        let package_archer = construct_group_twenty(&mut session);
        assert_eq!(
            (package_archer.max_life, package_archer.life),
            (
                PACKAGE_MAX_LIFE.cast_signed(),
                PACKAGE_MAX_LIFE.cast_signed()
            )
        );
        session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        let authored_frame_zero_hash = session.backend().campaign.state_hash();
        let authored_archer = construct_group_twenty(&mut session);
        assert_eq!(
            (authored_archer.max_life, authored_archer.life),
            (
                AUTHORED_MAX_LIFE.cast_signed(),
                AUTHORED_MAX_LIFE.cast_signed()
            )
        );
        session.reset(guard(&session)).unwrap();
        assert_eq!(
            session.backend().campaign.state_hash(),
            authored_frame_zero_hash
        );
        let reset_archer = construct_group_twenty(&mut session);
        assert_eq!(
            (reset_archer.max_life, reset_archer.life),
            (
                AUTHORED_MAX_LIFE.cast_signed(),
                AUTHORED_MAX_LIFE.cast_signed()
            )
        );

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this proves only the distinct real Level 2 group-20
        // producer consumes the same immutable Archer health binding after
        // reopen/reset. The Level 1 causal gate owns ordinary mailbox damage;
        // this test does not duplicate it or claim organic group activation,
        // Archer targeting/children, death, save/replay, or clients.
    }

    #[test]
    fn campaign_backend_binds_real_dragon_root_and_body_max_life_before_body_damage_after_reopen_and_reset()
     {
        const DRAGON_RECORD: u16 = 141;
        const GROUP_ONE_ROWS: &[u16] = &[141, 142];
        const PACKAGE_MAX_LIFE: u32 = 4_000;
        const AUTHORED_MAX_LIFE: u32 = 8_000;
        const DAMAGE: i32 = 4_500;

        let root = level_three_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let dragon =
            sim_definition::adapter::stable_entity_id("creature.dragon.at_79_143").unwrap();
        let target = base.level_definition().unwrap().entity(&dragon).unwrap();
        assert_eq!(
            target.creature_health().map(|health| (
                health.default_max_life(),
                health.max_life(),
                health.max_life_access(),
            )),
            Some((
                PACKAGE_MAX_LIFE,
                PACKAGE_MAX_LIFE,
                LevelEntityParameterAccess::SessionReset,
            ))
        );
        assert!(base.required_capabilities().iter().any(|requirement| {
            requirement.id.as_str() == DRAGON_MAX_LIFE_CAPABILITY_ID
                && requirement.exact_version == 1
        }));

        let workspace_root = temporary_workspace_path("campaign-dragon-max-life");
        let store =
            WorkspaceStore::create(&workspace_root, "campaign-dragon-max-life", base.clone())
                .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            81,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let change_set = ChangeSetBuilder::new(
            "campaign-dragon-max-life",
            &base,
            "raise-one-level-three-dragon-max-life",
            RequestedTiming::SessionReset,
        )
        .set_entity_max_life_from(dragon, PACKAGE_MAX_LIFE, AUTHORED_MAX_LIFE)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();

        let construct_group_one = |session: &mut GameSession<Mc2CampaignBackend>| {
            let compatibility = session.backend_mut().campaign.level2_compatibility_mut();
            compatibility
                .activate_authored_group_prerequisite_for_test(1)
                .unwrap();
            assert!(compatibility.dispatch_events().iter().any(|event| matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated {
                    group: 1,
                    authored_indices,
                    ..
                } if authored_indices.as_ref() == GROUP_ONE_ROWS
            )));
            let root = compatibility
                .economy()
                .authored_class5()
                .iter()
                .find(|source| source.authored_index == DRAGON_RECORD)
                .copied()
                .unwrap();
            let parts = compatibility
                .economy()
                .authored_dragon_parts()
                .iter()
                .filter(|part| part.entity_id == root.source_entity_index)
                .copied()
                .collect::<Vec<_>>();
            assert_eq!(parts.len(), 16);
            (root, parts)
        };
        let apply_body_damage = |session: &mut GameSession<Mc2CampaignBackend>, body_slot| {
            let attacker = session
                .backend()
                .campaign
                .level2_compatibility()
                .state
                .entity_index;
            assert!(
                session
                    .backend_mut()
                    .campaign
                    .level2_compatibility_mut()
                    .economy_mut()
                    .stage_dragon_mailbox_damage_for_test(body_slot, attacker, DAMAGE)
            );
            session
                .step(guard(session), GameCommand::default())
                .unwrap();
            session
                .backend()
                .campaign
                .level2_compatibility()
                .economy()
                .authored_dragon_parts()
                .iter()
                .find(|part| part.source_entity_index == body_slot)
                .copied()
                .unwrap()
        };

        let (package_dragon, package_parts) = construct_group_one(&mut session);
        assert_eq!(
            (package_dragon.max_life, package_dragon.life),
            (
                PACKAGE_MAX_LIFE.cast_signed(),
                PACKAGE_MAX_LIFE.cast_signed()
            )
        );
        assert!(package_parts.iter().all(|part| {
            (part.max_life, part.life)
                == (
                    PACKAGE_MAX_LIFE.cast_signed(),
                    PACKAGE_MAX_LIFE.cast_signed(),
                )
        }));
        let package_after_damage =
            apply_body_damage(&mut session, package_parts[0].source_entity_index);
        assert_eq!(package_after_damage.life, -500);

        session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        let authored_frame_zero_hash = session.backend().campaign.state_hash();
        let (authored_dragon, authored_parts) = construct_group_one(&mut session);
        assert_eq!(
            (authored_dragon.max_life, authored_dragon.life),
            (
                AUTHORED_MAX_LIFE.cast_signed(),
                AUTHORED_MAX_LIFE.cast_signed()
            )
        );
        assert!(authored_parts.iter().all(|part| {
            (part.max_life, part.life)
                == (
                    AUTHORED_MAX_LIFE.cast_signed(),
                    AUTHORED_MAX_LIFE.cast_signed(),
                )
        }));
        let authored_after_damage =
            apply_body_damage(&mut session, authored_parts[0].source_entity_index);
        assert_eq!(authored_after_damage.life, 3_500);

        session.reset(guard(&session)).unwrap();
        assert_eq!(
            session.backend().campaign.state_hash(),
            authored_frame_zero_hash
        );
        let (reset_dragon, reset_parts) = construct_group_one(&mut session);
        assert_eq!(
            (reset_dragon.max_life, reset_dragon.life),
            (
                AUTHORED_MAX_LIFE.cast_signed(),
                AUTHORED_MAX_LIFE.cast_signed()
            )
        );
        assert!(reset_parts.iter().all(|part| {
            (part.max_life, part.life)
                == (
                    AUTHORED_MAX_LIFE.cast_signed(),
                    AUTHORED_MAX_LIFE.cast_signed(),
                )
        }));

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this stages the already-proven real Level 3 group-1
        // producer and one body mailbox hit. It proves guarded per-instance
        // Dragon maximum-life publication, old-session isolation, all sixteen
        // source-copied body carriers, explicit reopen/reset construction, and
        // one ordinary body damage visit. It does not prove organic objective
        // progression, Dragon targeting/combat/death, body motion/fireballs,
        // other Dragon instances or creature families, save/replay, or clients.
    }

    #[test]
    fn campaign_packages_bind_only_level_three_dragons_to_the_health_capability() {
        let catalog = default_catalog().unwrap();
        for (root, expected_dragons) in [
            (level_one_descriptor_fixture_path(), 0_usize),
            (descriptor_fixture_path(), 0),
            (level_three_descriptor_fixture_path(), 5),
        ] {
            let definition = load_mc2_campaign(&root, &catalog).unwrap();
            let dragons = definition
                .level_definition()
                .unwrap()
                .entities()
                .iter()
                .filter(|entity| {
                    entity.kind()
                        == sim_definition::level_definition::LevelEntityKind::Creature(
                            CreatureKind::Dragon,
                        )
                })
                .collect::<Vec<_>>();
            assert_eq!(dragons.len(), expected_dragons);
            assert!(dragons.iter().all(|entity| {
                entity.creature_health().is_some_and(|health| {
                    health.default_max_life() == 4_000
                        && health.max_life() == 4_000
                        && health.max_life_access() == LevelEntityParameterAccess::SessionReset
                })
            }));
            assert_eq!(
                definition
                    .required_capabilities()
                    .iter()
                    .any(|requirement| {
                        requirement.id.as_str() == DRAGON_MAX_LIFE_CAPABILITY_ID
                            && requirement.exact_version == 1
                    }),
                expected_dragons > 0
            );
        }

        // No-Claim: this is only the closed first-three-package Dragon
        // descriptor/capability census. The separate causal gate owns one
        // real root/body construction and one ordinary body damage visit.
    }

    #[test]
    fn campaign_backend_binds_real_bee_max_life_before_mailbox_damage_after_reopen_and_reset() {
        const BEE_RECORD: u16 = 229;
        const GROUP_TWENTY_ONE_ROWS: &[u16] = &[229, 230, 231];
        const PACKAGE_MAX_LIFE: u32 = 3_000;
        const AUTHORED_MAX_LIFE: u32 = 6_000;
        const DAMAGE: i32 = 3_500;

        let root = descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let bee = sim_definition::adapter::stable_entity_id("creature.bee.at_97_17").unwrap();
        let target = base.level_definition().unwrap().entity(&bee).unwrap();
        assert_eq!(
            target.creature_health().map(|health| (
                health.default_max_life(),
                health.max_life(),
                health.max_life_access(),
            )),
            Some((
                PACKAGE_MAX_LIFE,
                PACKAGE_MAX_LIFE,
                LevelEntityParameterAccess::SessionReset,
            ))
        );
        let descriptor = base
            .level_creature_max_life_descriptor(
                &sim_definition::SemanticId::parse("creature.bee.at_97_17.max_life").unwrap(),
            )
            .unwrap();
        assert_eq!(
            (descriptor.minimum, descriptor.maximum),
            (Some(1), Some(32_767))
        );
        assert!(base.required_capabilities().iter().any(|requirement| {
            requirement.id.as_str() == BEE_MAX_LIFE_CAPABILITY_ID && requirement.exact_version == 1
        }));

        let workspace_root = temporary_workspace_path("campaign-bee-max-life");
        let store =
            WorkspaceStore::create(&workspace_root, "campaign-bee-max-life", base.clone()).unwrap();
        let before_rejection = store.state_bytes().unwrap();
        let out_of_range = ChangeSetBuilder::new(
            "campaign-bee-max-life",
            &base,
            "reject-source-width-bee-max-life",
            RequestedTiming::SessionReset,
        )
        .set_entity_max_life_from(bee.clone(), PACKAGE_MAX_LIFE, 32_768)
        .build()
        .unwrap();
        let rejection = store.validate(&catalog, out_of_range).unwrap();
        assert!(!rejection.accepted);
        assert_eq!(rejection.diagnostics[0].code, "OUT_OF_RANGE");
        assert_eq!(store.state_bytes().unwrap(), before_rejection);
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            83,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let change_set = ChangeSetBuilder::new(
            "campaign-bee-max-life",
            &base,
            "raise-one-level-two-bee-max-life",
            RequestedTiming::SessionReset,
        )
        .set_entity_max_life_from(bee, PACKAGE_MAX_LIFE, AUTHORED_MAX_LIFE)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();

        let construct_group_twenty_one = |session: &mut GameSession<Mc2CampaignBackend>| {
            let compatibility = session.backend_mut().campaign.level2_compatibility_mut();
            compatibility
                .activate_authored_group_prerequisite_for_test(21)
                .unwrap();
            assert!(compatibility.dispatch_events().iter().any(|event| matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated {
                    group: 21,
                    authored_indices,
                    ..
                } if authored_indices.as_ref() == GROUP_TWENTY_ONE_ROWS
            )));
            compatibility
                .economy()
                .bee_creatures()
                .iter()
                .find(|source| source.authored_index() == Some(BEE_RECORD))
                .copied()
                .unwrap()
        };
        let apply_damage = |session: &mut GameSession<Mc2CampaignBackend>, source_slot| {
            let attacker = session
                .backend()
                .campaign
                .level2_compatibility()
                .state
                .entity_index;
            assert!(
                session
                    .backend_mut()
                    .campaign
                    .level2_compatibility_mut()
                    .economy_mut()
                    .stage_bee_mailbox_damage_for_test(source_slot, attacker, DAMAGE)
            );
            session
                .step(guard(session), GameCommand::default())
                .unwrap();
            session
                .backend()
                .campaign
                .level2_compatibility()
                .economy()
                .bee_creatures()
                .iter()
                .find(|source| source.source_entity_index == source_slot)
                .copied()
                .unwrap()
        };
        let drive_to_action_seventeen = |session: &mut GameSession<Mc2CampaignBackend>,
                                         source_slot| {
            for _ in 0..160 {
                let source = session
                    .backend()
                    .campaign
                    .level2_compatibility()
                    .economy()
                    .bee_creatures()
                    .iter()
                    .find(|source| source.source_entity_index == source_slot)
                    .copied()
                    .unwrap();
                if source.action == 17 {
                    return source;
                }
                assert!(!matches!(source.action, 20 | 21));
                let player_position = if source.action == 23 {
                    let mut position = source.position;
                    crate::move_entity(&mut position, source.yaw, 0, 1_000);
                    position.z = source.position.z;
                    position
                } else {
                    crate::Position {
                        x: source.position.x.wrapping_add(20_000),
                        y: source.position.y.wrapping_add(20_000),
                        z: source.position.z,
                    }
                };
                let player = session
                    .backend()
                    .campaign
                    .level2_compatibility()
                    .state
                    .entity_index;
                session
                    .backend_mut()
                    .campaign
                    .level2_compatibility_mut()
                    .set_level2_entity_position(player, player_position)
                    .unwrap();
                session
                    .step(guard(session), GameCommand::default())
                    .unwrap();
            }
            panic!("production Bee did not return to action 17")
        };

        let package_bee = construct_group_twenty_one(&mut session);
        assert_eq!(
            (
                package_bee.max_life,
                package_bee.life,
                package_bee.mana,
                package_bee.action
            ),
            (3_000, 3_000, 1_500, 23)
        );
        let package_bee = drive_to_action_seventeen(&mut session, package_bee.source_entity_index);
        let package_after_damage = apply_damage(&mut session, package_bee.source_entity_index);
        assert_eq!(
            (package_after_damage.life, package_after_damage.action),
            (-500, 20)
        );

        session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        let authored_frame_zero_hash = session.backend().campaign.state_hash();
        let authored_bee = construct_group_twenty_one(&mut session);
        assert_eq!(
            (
                authored_bee.max_life,
                authored_bee.life,
                authored_bee.mana,
                authored_bee.action
            ),
            (6_000, 6_000, 3_000, 23)
        );
        let authored_bee =
            drive_to_action_seventeen(&mut session, authored_bee.source_entity_index);
        let authored_after_damage = apply_damage(&mut session, authored_bee.source_entity_index);
        assert_eq!(
            (authored_after_damage.life, authored_after_damage.action),
            (2_500, 18)
        );

        session.reset(guard(&session)).unwrap();
        assert_eq!(
            session.backend().campaign.state_hash(),
            authored_frame_zero_hash
        );
        let reset_bee = construct_group_twenty_one(&mut session);
        assert_eq!(
            (
                reset_bee.max_life,
                reset_bee.life,
                reset_bee.mana,
                reset_bee.action
            ),
            (6_000, 6_000, 3_000, 23)
        );

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this stages the already-proven real Level 2 group-21
        // producer and one action-17 mailbox hit. It proves guarded
        // per-instance Bee maximum-life publication, old-session isolation,
        // the source-derived half-life mana carrier, explicit reopen/reset
        // construction, and one ordinary damage visit. It does not prove
        // organic objective progression, Bee targeting/combat/death, Morph
        // Bees, other Bee instances or creature families, save/replay, or clients.
    }

    #[test]
    fn campaign_packages_bind_only_level_two_and_three_bees_to_the_health_capability() {
        let catalog = default_catalog().unwrap();
        for (root, expected_bees) in [
            (level_one_descriptor_fixture_path(), 0_usize),
            (descriptor_fixture_path(), 12),
            (level_three_descriptor_fixture_path(), 6),
        ] {
            let definition = load_mc2_campaign(&root, &catalog).unwrap();
            let bees = definition
                .level_definition()
                .unwrap()
                .entities()
                .iter()
                .filter(|entity| {
                    entity.kind()
                        == sim_definition::level_definition::LevelEntityKind::Creature(
                            CreatureKind::Bee,
                        )
                })
                .collect::<Vec<_>>();
            assert_eq!(bees.len(), expected_bees);
            assert!(bees.iter().all(|entity| {
                entity.creature_health().is_some_and(|health| {
                    health.default_max_life() == 3_000
                        && health.max_life() == 3_000
                        && health.max_life_access() == LevelEntityParameterAccess::SessionReset
                })
            }));
            assert_eq!(
                definition
                    .required_capabilities()
                    .iter()
                    .any(|requirement| {
                        requirement.id.as_str() == BEE_MAX_LIFE_CAPABILITY_ID
                            && requirement.exact_version == 1
                    }),
                expected_bees > 0
            );
        }

        // No-Claim: this is only the closed first-three-package Bee
        // descriptor/capability census. The separate causal gate owns one
        // real construction and ordinary damage visit.
    }

    #[test]
    fn campaign_backend_binds_real_level_three_reset_skeleton_max_life_before_ordinary_damage_after_reopen_and_reset()
     {
        const SKELETON_SLOT: u16 = 93;
        const PACKAGE_MAX_LIFE: u32 = 1_000;
        const AUTHORED_MAX_LIFE: u32 = 2_000;
        const DAMAGE: u16 = 1_500;

        let root = level_three_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let skeleton =
            sim_definition::adapter::stable_entity_id("creature.skeleton.at_75_140").unwrap();
        let target = base.level_definition().unwrap().entity(&skeleton).unwrap();
        assert_eq!(
            target.creature_health().map(|health| (
                health.default_max_life(),
                health.max_life(),
                health.max_life_access(),
            )),
            Some((
                PACKAGE_MAX_LIFE,
                PACKAGE_MAX_LIFE,
                LevelEntityParameterAccess::SessionReset,
            ))
        );
        assert!(base.required_capabilities().iter().any(|requirement| {
            requirement.id.as_str() == SKELETON_MAX_LIFE_CAPABILITY_ID
                && requirement.exact_version == 1
        }));

        let workspace_root = temporary_workspace_path("campaign-reset-skeleton-max-life");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-reset-skeleton-max-life",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            89,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let change_set = ChangeSetBuilder::new(
            "campaign-reset-skeleton-max-life",
            &base,
            "raise-one-level-three-reset-skeleton-max-life",
            RequestedTiming::SessionReset,
        )
        .set_entity_max_life_from(skeleton, PACKAGE_MAX_LIFE, AUTHORED_MAX_LIFE)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();

        let assert_health = |session: &GameSession<Mc2CampaignBackend>, expected: i32| {
            let source = &session
                .backend()
                .campaign
                .level2_compatibility()
                .session
                .state
                .live
                .economy
                .entities[usize::from(SKELETON_SLOT)];
            assert_eq!((source.class, source.model), (5, 9));
            assert_eq!((source.max_life, source.life), (expected, expected));
            assert_eq!(
                i32::from_le_bytes(source.record_bytes()[8..12].try_into().unwrap()),
                expected
            );
            assert_eq!(
                i32::from_le_bytes(source.record_bytes()[12..16].try_into().unwrap()),
                expected
            );
        };
        let drive_to_damage_visit = |session: &mut GameSession<Mc2CampaignBackend>| {
            for _ in 0..24 {
                let action = session
                    .backend()
                    .campaign
                    .level2_compatibility()
                    .session
                    .state
                    .live
                    .economy
                    .entities[usize::from(SKELETON_SLOT)]
                .action;
                if action == 79 {
                    return;
                }
                session
                    .step(guard(session), GameCommand::default())
                    .unwrap();
            }
            panic!("real reset Skeleton did not reach its damage-consuming action")
        };
        let apply_damage = |session: &mut GameSession<Mc2CampaignBackend>| {
            let attacker = session
                .backend()
                .campaign
                .level2_compatibility()
                .state
                .entity_index;
            session
                .backend_mut()
                .campaign
                .level2_compatibility_mut()
                .stage_imported_skeleton_fireball_damage_for_test(SKELETON_SLOT, attacker, DAMAGE)
                .unwrap();
            session
                .step(guard(session), GameCommand::default())
                .unwrap();
            session
                .backend()
                .campaign
                .level2_compatibility()
                .session
                .state
                .live
                .economy
                .entities[usize::from(SKELETON_SLOT)]
            .clone()
        };

        assert_health(&session, PACKAGE_MAX_LIFE.cast_signed());
        drive_to_damage_visit(&mut session);
        let package_after_damage = apply_damage(&mut session);
        assert_eq!(
            (package_after_damage.life, package_after_damage.action),
            (-500, 76)
        );

        session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        let authored_frame_zero_hash = session.backend().campaign.state_hash();
        assert_health(&session, AUTHORED_MAX_LIFE.cast_signed());
        drive_to_damage_visit(&mut session);
        let authored_after_damage = apply_damage(&mut session);
        assert_eq!(
            (authored_after_damage.life, authored_after_damage.action),
            (500, 74)
        );

        session.reset(guard(&session)).unwrap();
        assert_eq!(
            session.backend().campaign.state_hash(),
            authored_frame_zero_hash
        );
        assert_health(&session, AUTHORED_MAX_LIFE.cast_signed());

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this stages one already-arrived Fireball pulse at the
        // separately proven real Level 3 reset Skeleton. It proves guarded
        // per-instance maximum-life publication, old-session isolation,
        // typed/raw frame-zero binding, explicit reopen/reset, and one
        // ordinary model-9 damage visit. It does not prove organic projectile
        // travel, later death outputs/reclaim/objective completion, other
        // Skeleton instances, save/replay, or clients.
    }

    #[test]
    fn campaign_backend_binds_real_level_two_authored_skeleton_max_life_at_group_twenty_two_after_reopen_and_reset()
     {
        const SKELETON_RECORD: u16 = 239;
        const GROUP_TWENTY_TWO_ROWS: &[u16] = &[238, 239, 240, 241, 242, 243, 245];
        const PACKAGE_MAX_LIFE: u32 = 1_000;
        const AUTHORED_MAX_LIFE: u32 = 2_000;

        let root = descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let skeleton =
            sim_definition::adapter::stable_entity_id("creature.skeleton.at_140_220").unwrap();
        let workspace_root = temporary_workspace_path("campaign-authored-skeleton-max-life");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-authored-skeleton-max-life",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            91,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let change_set = ChangeSetBuilder::new(
            "campaign-authored-skeleton-max-life",
            &base,
            "raise-one-level-two-authored-skeleton-max-life",
            RequestedTiming::SessionReset,
        )
        .set_entity_max_life_from(skeleton, PACKAGE_MAX_LIFE, AUTHORED_MAX_LIFE)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();

        let construct_group_twenty_two = |session: &mut GameSession<Mc2CampaignBackend>| {
            let compatibility = session.backend_mut().campaign.level2_compatibility_mut();
            compatibility
                .activate_authored_group_prerequisite_for_test(22)
                .unwrap();
            assert!(compatibility.dispatch_events().iter().any(|event| matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated { group: 22, .. }
            )));
            assert_eq!(
                compatibility
                    .economy()
                    .authored_class5()
                    .iter()
                    .map(|source| source.authored_index)
                    .collect::<Vec<_>>(),
                GROUP_TWENTY_TWO_ROWS
            );
            compatibility
                .economy()
                .authored_class5()
                .iter()
                .find(|source| source.authored_index == SKELETON_RECORD)
                .copied()
                .unwrap()
        };

        let package_skeleton = construct_group_twenty_two(&mut session);
        assert_eq!(
            (package_skeleton.max_life, package_skeleton.life),
            (
                PACKAGE_MAX_LIFE.cast_signed(),
                PACKAGE_MAX_LIFE.cast_signed()
            )
        );
        session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        let authored_frame_zero_hash = session.backend().campaign.state_hash();
        let authored_skeleton = construct_group_twenty_two(&mut session);
        assert_eq!(
            (authored_skeleton.max_life, authored_skeleton.life),
            (
                AUTHORED_MAX_LIFE.cast_signed(),
                AUTHORED_MAX_LIFE.cast_signed()
            )
        );
        session.reset(guard(&session)).unwrap();
        assert_eq!(
            session.backend().campaign.state_hash(),
            authored_frame_zero_hash
        );
        let reset_skeleton = construct_group_twenty_two(&mut session);
        assert_eq!(
            (reset_skeleton.max_life, reset_skeleton.life),
            (
                AUTHORED_MAX_LIFE.cast_signed(),
                AUTHORED_MAX_LIFE.cast_signed()
            )
        );

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this stages the already-proven real Level 2 group-22
        // producer and proves its authored Skeleton constructor consumes the
        // same immutable health binding after reopen/reset. The Level 3 gate
        // owns ordinary damage; this test does not duplicate combat/death,
        // organic objective progression, save/replay, or clients.
    }

    #[test]
    fn campaign_packages_bind_only_level_two_and_three_skeletons_to_the_health_capability() {
        let catalog = default_catalog().unwrap();
        for (root, expected_skeletons) in [
            (level_one_descriptor_fixture_path(), 0_usize),
            (descriptor_fixture_path(), 6),
            (level_three_descriptor_fixture_path(), 11),
        ] {
            let definition = load_mc2_campaign(&root, &catalog).unwrap();
            let skeletons = definition
                .level_definition()
                .unwrap()
                .entities()
                .iter()
                .filter(|entity| {
                    entity.kind()
                        == sim_definition::level_definition::LevelEntityKind::Creature(
                            CreatureKind::Skeleton,
                        )
                })
                .collect::<Vec<_>>();
            assert_eq!(skeletons.len(), expected_skeletons);
            assert!(skeletons.iter().all(|entity| {
                entity.creature_health().is_some_and(|health| {
                    health.default_max_life() == 1_000
                        && health.max_life() == 1_000
                        && health.max_life_access() == LevelEntityParameterAccess::SessionReset
                })
            }));
            assert_eq!(
                definition
                    .required_capabilities()
                    .iter()
                    .any(|requirement| {
                        requirement.id.as_str() == SKELETON_MAX_LIFE_CAPABILITY_ID
                            && requirement.exact_version == 1
                    }),
                expected_skeletons > 0
            );
        }

        // No-Claim: this is only the closed first-three-package Skeleton
        // descriptor/capability census. The two focused production gates own
        // the distinct authored constructor and reset-live damage consumers.
    }

    #[test]
    fn campaign_backend_moves_real_level_three_reset_skeleton_before_ordinary_model9_visit_after_reopen()
     {
        const SKELETON_SLOT: u16 = 93;

        let root = level_three_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let skeleton =
            sim_definition::adapter::stable_entity_id("creature.skeleton.at_75_140").unwrap();
        let package_tile = TilePosition { x: 75, y: 140 };
        let authored_tile = TilePosition { x: 90, y: 150 };
        let package_entity = base.level_definition().unwrap().entity(&skeleton).unwrap();
        assert_eq!(
            package_entity.kind(),
            LevelEntityKind::Creature(CreatureKind::Skeleton)
        );
        assert_eq!(package_entity.position(), package_tile);
        assert!(package_entity.activation_group().is_none());
        assert_eq!(
            package_entity.position_access(),
            LevelEntityPositionAccess::SessionReset
        );

        let workspace_root = temporary_workspace_path("campaign-reset-skeleton-placement");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-reset-skeleton-placement",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            79,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let package_source = session.backend().campaign.level2_compatibility().entities()
            [usize::from(SKELETON_SLOT)]
        .clone();
        assert_eq!(
            (
                package_source.class,
                package_source.model,
                package_source.action
            ),
            (5, 9, 72)
        );
        assert_eq!(
            (
                package_source.position.x >> 8,
                package_source.position.y >> 8
            ),
            (package_tile.x, package_tile.y)
        );

        let change_set = ChangeSetBuilder::new(
            "campaign-reset-skeleton-placement",
            &base,
            "move-level-three-reset-skeleton",
            RequestedTiming::SessionReset,
        )
        .move_entity_from(skeleton.clone(), package_tile, authored_tile)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();
        assert_eq!(published.revision(), 2);
        assert_eq!(session.identity().definition_hash, base.definition_hash());
        assert_eq!(
            session.backend().campaign.level2_compatibility().entities()
                [usize::from(SKELETON_SLOT)]
            .position,
            package_source.position,
            "publication must not mutate the already-open package authority"
        );

        let reopened = session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        assert_eq!(reopened.identity.generation, 2);
        assert_eq!(reopened.identity.definition_revision, 2);
        let authored_position = crate::Position {
            x: (authored_tile.x << 8) | (package_source.position.x & 0xff),
            y: (authored_tile.y << 8) | (package_source.position.y & 0xff),
            z: package_source.position.z,
        };
        let authored_reset_hash = session.backend().campaign.state_hash();
        {
            let compatibility = session.backend().campaign.level2_compatibility();
            let source = &compatibility.entities()[usize::from(SKELETON_SLOT)];
            assert_eq!(source.position, authored_position);
            assert_eq!((source.class, source.model, source.action), (5, 9, 72));
            assert_eq!(
                compatibility
                    .session
                    .state
                    .live
                    .economy
                    .map_chains()
                    .tile_of(SKELETON_SLOT),
                u16::from_le_bytes([authored_tile.x as u8, authored_tile.y as u8])
            );
        }

        for _ in 0..24 {
            if session.backend().campaign.level2_compatibility().entities()
                [usize::from(SKELETON_SLOT)]
            .action
                == 79
            {
                break;
            }
            session
                .step(guard(&session), GameCommand::default())
                .unwrap();
        }
        let movement_start = session.backend().campaign.level2_compatibility().entities()
            [usize::from(SKELETON_SLOT)]
        .clone();
        assert_eq!(
            movement_start.action, 79,
            "the real reset Skeleton must reach its ordinary movement callback"
        );
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        let compatibility = session.backend().campaign.level2_compatibility();
        let moved = &compatibility.entities()[usize::from(SKELETON_SLOT)];
        assert_eq!((moved.class, moved.model), (5, 9));
        assert!(moved.active && !moved.drawing_disabled());
        assert_ne!(moved.position, movement_start.position);
        assert_eq!(moved.phase, movement_start.phase.wrapping_add(1));
        assert!(
            compatibility
                .session
                .state
                .live
                .economy
                .map_chains()
                .is_linked(SKELETON_SLOT)
        );
        assert!(
            session
                .backend()
                .campaign
                .events()
                .iter()
                .all(|event| !matches!(
                    event,
                    Level2DispatchEvent::Error {
                        source_slot: SKELETON_SLOT,
                        ..
                    }
                ))
        );

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.backend().campaign.state_hash(), authored_reset_hash);
        let reset = &session.backend().campaign.level2_compatibility().entities()
            [usize::from(SKELETON_SLOT)];
        assert_eq!(reset.position, authored_position);
        assert_eq!((reset.class, reset.model, reset.action), (5, 9, 72));

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this proves one reset-live Level 3 complete-lifecycle
        // Skeleton placement crosses guarded publication, explicit reopen,
        // its ordinary model-9 movement/map-chain consumer, and reset. It does
        // not re-prove Skeleton combat/death/objective cleanup, grouped creature
        // construction, Goat/Worm/Villager/Trader placement, save/replay, or
        // Unity presentation.
    }

    #[test]
    fn campaign_backend_moves_real_level_one_dynamic_castle_pickup_through_group_one_after_reopen()
    {
        const PICKUP_RECORD: u16 = 31;
        const PRODUCER_SLOT: u16 = 48;
        const CASTLE_FAMILY: u8 = 2;
        const GROUP_ONE_ROWS: &[u16] = &[
            31, 99, 104, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 168, 169, 170, 171, 172,
            173, 174, 175,
        ];

        let root = level_one_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let castle =
            sim_definition::adapter::stable_entity_id("spell_pickup.castle.at_194_213").unwrap();
        let package_tile = TilePosition { x: 194, y: 213 };
        let authored_tile = TilePosition { x: 180, y: 180 };
        let package_entity = base.level_definition().unwrap().entity(&castle).unwrap();
        assert_eq!(
            package_entity.kind(),
            LevelEntityKind::SpellPickup(SpellKind::Castle)
        );
        assert_eq!(package_entity.position(), package_tile);
        assert_eq!(
            package_entity.position_access(),
            LevelEntityPositionAccess::SessionReset
        );

        let workspace_root = temporary_workspace_path("campaign-castle-pickup-placement");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-castle-pickup-placement",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            73,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();

        let change_set = ChangeSetBuilder::new(
            "campaign-castle-pickup-placement",
            &base,
            "move-level-one-castle-pickup",
            RequestedTiming::SessionReset,
        )
        .move_entity_from(castle.clone(), package_tile, authored_tile)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();
        assert_eq!(published.revision(), 2);
        assert_eq!(
            published
                .level_definition()
                .unwrap()
                .entity(&castle)
                .unwrap()
                .position(),
            authored_tile
        );

        let move_player_outside_group_one = |session: &mut GameSession<Mc2CampaignBackend>| {
            let compatibility = session.backend().campaign.level2_compatibility();
            let player = compatibility.state.entity_index;
            let outside = crate::Position {
                x: 0,
                y: 0,
                z: compatibility.state.position.z,
            };
            session
                .backend_mut()
                .campaign
                .level2_compatibility_mut()
                .set_level2_entity_position(player, outside)
                .unwrap();
        };
        let step_until_group_one = |session: &mut GameSession<Mc2CampaignBackend>| {
            for _ in 0..8 {
                session
                    .step(guard(session), GameCommand::default())
                    .unwrap();
            }
            assert!(
                session
                    .backend()
                    .campaign
                    .events()
                    .iter()
                    .any(|event| matches!(
                        event,
                        Level2DispatchEvent::AuthoredGroupActivated {
                            source_slot: PRODUCER_SLOT,
                            group: 1,
                            authored_indices,
                            ..
                        } if authored_indices.as_ref() == GROUP_ONE_ROWS
                    ))
            );
        };

        move_player_outside_group_one(&mut session);
        step_until_group_one(&mut session);
        let package_pickup = session
            .backend()
            .campaign
            .level2_compatibility()
            .economy()
            .spell_pickups()
            .iter()
            .find(|pickup| pickup.authored_index == PICKUP_RECORD)
            .copied()
            .unwrap();
        assert_eq!(
            (
                package_pickup.position.x >> 8,
                package_pickup.position.y >> 8
            ),
            (package_tile.x, package_tile.y),
            "publication must not mutate the already-open package authority"
        );
        assert_eq!(session.identity().definition_hash, base.definition_hash());

        let reopened = session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        assert_eq!(reopened.identity.generation, 2);
        assert_eq!(reopened.identity.definition_revision, 2);
        let compatibility = session.backend().campaign.level2_compatibility();
        assert_eq!(
            compatibility
                .mission()
                .authored_features()
                .iter()
                .find(|feature| feature.index == PICKUP_RECORD)
                .map(|feature| (feature.tile_x, feature.tile_y)),
            Some((authored_tile.x, authored_tile.y))
        );
        assert!(compatibility.shared_spells().dynamic_pickups().is_empty());
        let authored_reset_hash = session.backend().campaign.state_hash();

        move_player_outside_group_one(&mut session);
        step_until_group_one(&mut session);
        let compatibility = session.backend().campaign.level2_compatibility();
        let authored_pickup = compatibility
            .economy()
            .spell_pickups()
            .iter()
            .find(|pickup| pickup.authored_index == PICKUP_RECORD)
            .copied()
            .unwrap();
        let source_slot = authored_pickup.source_entity_index;
        assert_eq!(authored_pickup.family, CASTLE_FAMILY);
        assert_eq!(
            (
                authored_pickup.position.x >> 8,
                authored_pickup.position.y >> 8
            ),
            (authored_tile.x, authored_tile.y)
        );
        let shared_pickup = compatibility
            .shared_spells()
            .dynamic_pickups()
            .iter()
            .find(|pickup| pickup.index == source_slot)
            .unwrap();
        assert_eq!(shared_pickup.position, authored_pickup.position);
        assert_eq!(shared_pickup.model, CASTLE_FAMILY);
        assert_eq!(
            compatibility.economy().map_chains().tile_of(source_slot),
            u16::from_le_bytes([authored_tile.x as u8, authored_tile.y as u8])
        );

        let player = compatibility.state.entity_index;
        let package_position = crate::Position {
            x: package_tile.x.wrapping_shl(8).wrapping_add(128),
            y: package_tile.y.wrapping_shl(8).wrapping_add(128),
            z: authored_pickup.position.z,
        };
        session
            .backend_mut()
            .campaign
            .level2_compatibility_mut()
            .set_level2_entity_position(player, package_position)
            .unwrap();
        for _ in 0..4 {
            session
                .step(guard(&session), GameCommand::default())
                .unwrap();
        }
        assert!(
            !session
                .backend()
                .campaign
                .events()
                .iter()
                .any(|event| matches!(
                    event,
                    Level2DispatchEvent::SpellAcquisition {
                        source_slot: slot,
                        family: CASTLE_FAMILY,
                        result: Some(_),
                        ..
                    } if *slot == source_slot
                )),
            "a definition-only fake would still acquire at the package position"
        );

        session
            .backend_mut()
            .campaign
            .level2_compatibility_mut()
            .set_level2_entity_position(player, authored_pickup.position)
            .unwrap();
        for _ in 0..4 {
            session
                .step(guard(&session), GameCommand::default())
                .unwrap();
            if session.backend().campaign.events().iter().any(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::SpellAcquisition {
                        source_slot: slot,
                        family: CASTLE_FAMILY,
                        result: Some(_),
                        ..
                    } if *slot == source_slot
                )
            }) {
                break;
            }
        }
        assert!(
            session
                .backend()
                .campaign
                .events()
                .iter()
                .any(|event| matches!(
                    event,
                    Level2DispatchEvent::SpellAcquisition {
                        source_slot: slot,
                        family: CASTLE_FAMILY,
                        result: Some(_),
                        ..
                    } if *slot == source_slot
                ))
        );
        assert_eq!(
            session.backend().campaign.level2_compatibility().players()[0].enabled_spell_holders
                [usize::from(CASTLE_FAMILY)],
            source_slot,
            "the ordinary acquisition callback must publish the authored pickup as the player's Castle holder"
        );

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.backend().campaign.state_hash(), authored_reset_hash);
        let reset = session.backend().campaign.level2_compatibility();
        assert!(reset.shared_spells().dynamic_pickups().is_empty());
        assert_eq!(
            reset
                .mission()
                .authored_features()
                .iter()
                .find(|feature| feature.index == PICKUP_RECORD)
                .map(|feature| (feature.tile_x, feature.tile_y)),
            Some((authored_tile.x, authored_tile.y))
        );

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this proves the real Level 1 Castle authored-feature row
        // crosses guarded publication, explicit reopen, ordinary group-1
        // construction, map/shared-spell projection, position-sensitive
        // acquisition, and reset-template retention. The exact package census
        // makes the other four dynamic spell pickups authorable through this
        // same typed binding, but this test does not re-prove their distinct
        // package-group producers, later spell behavior, save/replay, or Unity.
    }

    #[test]
    fn campaign_backend_moves_real_level_one_dynamic_tree_through_group_one_after_reopen() {
        const TREE_RECORD: u16 = 155;
        const PRODUCER_SLOT: u16 = 48;
        const GROUP_ONE_ROWS: &[u16] = &[
            31, 99, 104, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 168, 169, 170, 171, 172,
            173, 174, 175,
        ];

        let root = level_one_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let tree = sim_definition::adapter::stable_entity_id("scenery.tree.at_110_96").unwrap();
        let package_tile = TilePosition { x: 110, y: 96 };
        let authored_tile = TilePosition { x: 130, y: 120 };
        let package_entity = base.level_definition().unwrap().entity(&tree).unwrap();
        assert_eq!(
            package_entity.kind(),
            LevelEntityKind::Scenery(SceneryKind::Tree)
        );
        assert_eq!(package_entity.position(), package_tile);
        assert_eq!(
            package_entity.position_access(),
            LevelEntityPositionAccess::SessionReset
        );

        let workspace_root = temporary_workspace_path("campaign-dynamic-tree-placement");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-dynamic-tree-placement",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            75,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let change_set = ChangeSetBuilder::new(
            "campaign-dynamic-tree-placement",
            &base,
            "move-level-one-dynamic-tree",
            RequestedTiming::SessionReset,
        )
        .move_entity_from(tree.clone(), package_tile, authored_tile)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();

        let run_group_one = |session: &mut GameSession<Mc2CampaignBackend>| {
            let compatibility = session.backend().campaign.level2_compatibility();
            let player = compatibility.state.entity_index;
            let outside = crate::Position {
                x: 0,
                y: 0,
                z: compatibility.state.position.z,
            };
            session
                .backend_mut()
                .campaign
                .level2_compatibility_mut()
                .set_level2_entity_position(player, outside)
                .unwrap();
            for _ in 0..8 {
                session
                    .step(guard(session), GameCommand::default())
                    .unwrap();
            }
            assert!(
                session
                    .backend()
                    .campaign
                    .events()
                    .iter()
                    .any(|event| matches!(
                        event,
                        Level2DispatchEvent::AuthoredGroupActivated {
                            source_slot: PRODUCER_SLOT,
                            group: 1,
                            authored_indices,
                            ..
                        } if authored_indices.as_ref() == GROUP_ONE_ROWS
                    ))
            );
        };
        let assert_tree_near = |tree: &crate::native_world::Level2AuthoredTreeSource,
                                tile: TilePosition| {
            let center_x = tile.x.wrapping_shl(8).wrapping_add(128);
            let center_y = tile.y.wrapping_shl(8).wrapping_add(128);
            assert!(tree.position.x.abs_diff(center_x) <= 32);
            assert!(tree.position.y.abs_diff(center_y) <= 32);
        };

        run_group_one(&mut session);
        let package_tree = session
            .backend()
            .campaign
            .level2_compatibility()
            .economy()
            .authored_trees()
            .iter()
            .find(|source| source.authored_index == TREE_RECORD)
            .copied()
            .unwrap();
        assert_tree_near(&package_tree, package_tile);
        assert_eq!(session.identity().definition_hash, base.definition_hash());

        let reopened = session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        assert_eq!(reopened.identity.generation, 2);
        assert_eq!(reopened.identity.definition_revision, 2);
        let compatibility = session.backend().campaign.level2_compatibility();
        assert_eq!(
            compatibility
                .mission()
                .authored_features()
                .iter()
                .find(|feature| feature.index == TREE_RECORD)
                .map(|feature| (feature.tile_x, feature.tile_y)),
            Some((authored_tile.x, authored_tile.y))
        );
        assert!(compatibility.economy().authored_trees().is_empty());
        let authored_reset_hash = session.backend().campaign.state_hash();

        run_group_one(&mut session);
        let compatibility = session.backend().campaign.level2_compatibility();
        let authored_tree = compatibility
            .economy()
            .authored_trees()
            .iter()
            .find(|source| source.authored_index == TREE_RECORD)
            .copied()
            .unwrap();
        assert_tree_near(&authored_tree, authored_tile);
        let package_center = crate::Position {
            x: package_tile.x.wrapping_shl(8).wrapping_add(128),
            y: package_tile.y.wrapping_shl(8).wrapping_add(128),
            z: authored_tree.position.z,
        };
        assert!(authored_tree.position.x.abs_diff(package_center.x) > 32);
        assert!(authored_tree.position.y.abs_diff(package_center.y) > 32);
        assert_eq!(
            authored_tree.position.z,
            crate::FlightWorld::ground_height(
                compatibility.canonical_terrain(),
                authored_tree.position,
            )
        );
        assert!(
            compatibility
                .economy()
                .map_chains()
                .entity_chain(
                    (authored_tree.position.x >> 8) as u8,
                    (authored_tree.position.y >> 8) as u8,
                )
                .contains(&authored_tree.source_entity_index)
        );
        let receipts = compatibility.cf10_production_schedule_receipts();
        assert!(receipts.iter().any(|receipt| matches!(
            receipt,
            crate::campaign::schedule::ScheduleReceipt::CallbackCompleted {
                slot,
                ..
            } if *slot == authored_tree.source_entity_index
        )));
        assert!(receipts.iter().any(|receipt| matches!(
            receipt,
            crate::campaign::schedule::ScheduleReceipt::EpilogueCompleted {
                slot,
                ..
            } if *slot == authored_tree.source_entity_index
        )));

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.backend().campaign.state_hash(), authored_reset_hash);
        let reset = session.backend().campaign.level2_compatibility();
        assert!(reset.economy().authored_trees().is_empty());
        assert_eq!(
            reset
                .mission()
                .authored_features()
                .iter()
                .find(|feature| feature.index == TREE_RECORD)
                .map(|feature| (feature.tile_x, feature.tile_y)),
            Some((authored_tile.x, authored_tile.y))
        );

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this proves one real Level 1 authored Tree crosses guarded
        // publication, explicit reopen, the ordinary group-1 constructor,
        // numeric callback/epilogue, terrain ground and map ownership, plus
        // reset-template retention. The exact package census authorizes every
        // other Tree/Stone through the same reset-versus-authored binding, but
        // the player is staged away from unrelated overlap consumers and this
        // test does not re-prove their distinct groups, damage/death, water
        // hiding, save/replay, or Unity presentation.
    }

    #[test]
    fn campaign_backend_moves_real_level_three_shield_pickup_only_on_reopen_and_acquires_at_authored_position()
     {
        const PICKUP_SLOT: u16 = 124;
        const SHIELD_FAMILY: u8 = 6;
        let root = level_three_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let shield =
            sim_definition::adapter::stable_entity_id("spell_pickup.shield.at_85_85").unwrap();
        let package_tile = TilePosition { x: 85, y: 85 };
        let authored_tile = TilePosition { x: 92, y: 91 };
        let package_entity = base.level_definition().unwrap().entity(&shield).unwrap();
        assert_eq!(package_entity.position(), package_tile);
        assert_eq!(
            package_entity.position_access(),
            LevelEntityPositionAccess::SessionReset
        );

        let workspace_root = temporary_workspace_path("campaign-shield-placement");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-shield-placement-authoring",
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
        let package_position = session.backend().campaign.level2_compatibility().entities()
            [usize::from(PICKUP_SLOT)]
        .position;
        assert_eq!(
            session.backend().campaign.level2_compatibility().entities()[usize::from(PICKUP_SLOT)]
                .position,
            package_position
        );

        let change_set = ChangeSetBuilder::new(
            "campaign-shield-placement-authoring",
            &base,
            "move-level-three-shield",
            RequestedTiming::SessionReset,
        )
        .move_entity_from(shield.clone(), package_tile, authored_tile)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();
        assert_eq!(published.revision(), 2);
        assert_eq!(
            published
                .level_definition()
                .unwrap()
                .entity(&shield)
                .unwrap()
                .position(),
            authored_tile
        );
        assert_eq!(session.identity().definition_hash, base.definition_hash());
        assert_eq!(
            session.backend().campaign.level2_compatibility().entities()[usize::from(PICKUP_SLOT)]
                .position,
            package_position,
            "publication must not mutate the already-open package authority"
        );

        session
            .reopen(guard(&session), Arc::new(published.clone()))
            .unwrap();
        let authored_position = crate::Position {
            x: (authored_tile.x << 8) | (package_position.x & 0xff),
            y: (authored_tile.y << 8) | (package_position.y & 0xff),
            z: package_position.z,
        };
        let authored_reset_hash = session.backend().campaign.state_hash();
        {
            let live = &session.backend().campaign.level2.session.state.live;
            assert_eq!(
                live.economy.entities[usize::from(PICKUP_SLOT)].position,
                authored_position
            );
            assert_eq!(
                live.shared_spells
                    .pickups()
                    .iter()
                    .find(|pickup| pickup.index == PICKUP_SLOT)
                    .unwrap()
                    .position,
                authored_position
            );
            assert_eq!(
                live.economy.map_chains().tile_of(PICKUP_SLOT),
                u16::from_le_bytes([authored_tile.x as u8, authored_tile.y as u8])
            );
        }

        let player = session
            .backend()
            .campaign
            .level2_compatibility()
            .session
            .state
            .live
            .core
            .state
            .entity_index;
        session
            .backend_mut()
            .campaign
            .level2_compatibility_mut()
            .set_level2_entity_position(player, package_position)
            .unwrap();
        for _ in 0..4 {
            session
                .step(guard(&session), GameCommand::default())
                .unwrap();
        }
        assert_eq!(
            session.backend().campaign.level2_compatibility().entities()[usize::from(PICKUP_SLOT)]
                .action,
            19,
            "a definition-only fake would still acquire at the package position"
        );
        assert!(!session.backend().campaign.events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::SpellAcquisition {
                    source_slot: PICKUP_SLOT,
                    family: SHIELD_FAMILY,
                    result: Some(_),
                    ..
                }
            )
        }));

        session
            .backend_mut()
            .campaign
            .level2_compatibility_mut()
            .set_level2_entity_position(player, authored_position)
            .unwrap();
        for _ in 0..4 {
            session
                .step(guard(&session), GameCommand::default())
                .unwrap();
            if session.backend().campaign.level2_compatibility().entities()
                [usize::from(PICKUP_SLOT)]
            .action
                == 18
            {
                break;
            }
        }
        assert!(session.backend().campaign.events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::SpellAcquisition {
                    source_slot: PICKUP_SLOT,
                    family: SHIELD_FAMILY,
                    result: Some(_),
                    ..
                }
            )
        }));
        assert_eq!(
            session.backend().campaign.level2_compatibility().entities()[usize::from(PICKUP_SLOT)]
                .action,
            18
        );

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.backend().campaign.state_hash(), authored_reset_hash);
        let reset_live = &session.backend().campaign.level2.session.state.live;
        assert_eq!(
            reset_live.economy.entities[usize::from(PICKUP_SLOT)].position,
            authored_position
        );
        assert_eq!(
            reset_live
                .shared_spells
                .pickups()
                .iter()
                .find(|pickup| pickup.index == PICKUP_SLOT)
                .unwrap()
                .position,
            authored_position
        );
        assert_eq!(
            reset_live.economy.entities[usize::from(PICKUP_SLOT)].action,
            19
        );

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this proves one reset-live Level 3 Shield pickup placement,
        // ordinary overlap acquisition, explicit reopen, and reset retention.
        // It does not make generated pickups, other placements, spell tuning,
        // later holder behavior, save/replay, or Unity presentation authorable.
    }

    #[test]
    fn campaign_backend_moves_real_level_three_stone_and_runs_ordinary_ground_consumer() {
        const STONE_SLOT: u16 = 220;
        let root = level_three_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let stone = sim_definition::adapter::stable_entity_id("scenery.stone.at_219_93").unwrap();
        let package_tile = TilePosition { x: 219, y: 93 };
        let authored_tile = TilePosition { x: 210, y: 90 };
        let package_entity = base.level_definition().unwrap().entity(&stone).unwrap();
        assert_eq!(package_entity.position(), package_tile);
        assert_eq!(
            package_entity.position_access(),
            LevelEntityPositionAccess::SessionReset
        );

        let workspace_root = temporary_workspace_path("campaign-stone-placement");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-stone-placement-authoring",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            73,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let package_position = session.backend().campaign.level2_compatibility().entities()
            [usize::from(STONE_SLOT)]
        .position;

        let change_set = ChangeSetBuilder::new(
            "campaign-stone-placement-authoring",
            &base,
            "move-level-three-stone",
            RequestedTiming::SessionReset,
        )
        .move_entity_from(stone.clone(), package_tile, authored_tile)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();
        assert_eq!(published.revision(), 2);
        assert_eq!(session.identity().definition_hash, base.definition_hash());
        assert_eq!(
            session.backend().campaign.level2_compatibility().entities()[usize::from(STONE_SLOT)]
                .position,
            package_position,
            "publication must not mutate the already-open package authority"
        );

        session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        let authored_position = crate::Position {
            x: (authored_tile.x << 8) | (package_position.x & 0xff),
            y: (authored_tile.y << 8) | (package_position.y & 0xff),
            z: package_position.z,
        };
        let compatibility = session.backend().campaign.level2_compatibility();
        let target_ground =
            crate::FlightWorld::ground_height(compatibility.canonical_terrain(), authored_position);
        assert_ne!(
            target_ground, package_position.z,
            "the authored tile must discriminate the ordinary Stone ground callback"
        );
        assert_eq!(
            compatibility.entities()[usize::from(STONE_SLOT)].position,
            authored_position
        );
        assert_eq!(
            compatibility
                .session
                .state
                .live
                .economy
                .map_chains()
                .tile_of(STONE_SLOT),
            u16::from_le_bytes([authored_tile.x as u8, authored_tile.y as u8])
        );
        let authored_reset_hash = session.backend().campaign.state_hash();

        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        let stepped =
            &session.backend().campaign.level2_compatibility().entities()[usize::from(STONE_SLOT)];
        assert_eq!(stepped.position.x, authored_position.x);
        assert_eq!(stepped.position.y, authored_position.y);
        assert_eq!(stepped.position.z, target_ground);
        assert_ne!(session.backend().campaign.state_hash(), authored_reset_hash);

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.backend().campaign.state_hash(), authored_reset_hash);
        let reset =
            &session.backend().campaign.level2_compatibility().entities()[usize::from(STONE_SLOT)];
        assert_eq!(reset.position, authored_position);
        assert_eq!(reset.action, 3);

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this proves one reset-live Level 3 ordinary Stone placement,
        // map relocation, explicit reopen/reset, and its ground-height callback.
        // It does not make authored-group Stones, trees, buildings, creatures,
        // damage/destruction, save/replay, or Unity presentation authorable.
    }

    #[test]
    fn campaign_backend_moves_real_level_three_tree_and_runs_ordinary_ground_consumer() {
        const TREE_SLOT: u16 = 95;
        let root = level_three_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let tree = sim_definition::adapter::stable_entity_id("scenery.tree.at_100_149").unwrap();
        let package_tile = TilePosition { x: 100, y: 149 };
        let authored_tile = TilePosition { x: 110, y: 140 };
        let package_entity = base.level_definition().unwrap().entity(&tree).unwrap();
        assert_eq!(package_entity.position(), package_tile);
        assert_eq!(
            package_entity.position_access(),
            LevelEntityPositionAccess::SessionReset
        );

        let workspace_root = temporary_workspace_path("campaign-tree-placement");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-tree-placement-authoring",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            74,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let package_position = session.backend().campaign.level2_compatibility().entities()
            [usize::from(TREE_SLOT)]
        .position;

        let change_set = ChangeSetBuilder::new(
            "campaign-tree-placement-authoring",
            &base,
            "move-level-three-tree",
            RequestedTiming::SessionReset,
        )
        .move_entity_from(tree.clone(), package_tile, authored_tile)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();
        assert_eq!(session.identity().definition_hash, base.definition_hash());
        assert_eq!(
            session.backend().campaign.level2_compatibility().entities()[usize::from(TREE_SLOT)]
                .position,
            package_position
        );

        session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        let authored_position = crate::Position {
            x: (authored_tile.x << 8) | (package_position.x & 0xff),
            y: (authored_tile.y << 8) | (package_position.y & 0xff),
            z: package_position.z,
        };
        let compatibility = session.backend().campaign.level2_compatibility();
        let target_ground =
            crate::FlightWorld::ground_height(compatibility.canonical_terrain(), authored_position);
        assert_ne!(target_ground, package_position.z);
        assert_ne!(
            crate::FlightWorld::material_mask(compatibility.canonical_terrain(), authored_position),
            crate::WATER_AMBIENCE_MASK,
            "the focused target keeps water-hide allocation outside this proof"
        );
        assert_eq!(
            compatibility.entities()[usize::from(TREE_SLOT)].position,
            authored_position
        );
        assert_eq!(
            compatibility
                .session
                .state
                .live
                .economy
                .map_chains()
                .tile_of(TREE_SLOT),
            u16::from_le_bytes([authored_tile.x as u8, authored_tile.y as u8])
        );
        let authored_reset_hash = session.backend().campaign.state_hash();

        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        let stepped =
            &session.backend().campaign.level2_compatibility().entities()[usize::from(TREE_SLOT)];
        assert_eq!(stepped.position.x, authored_position.x);
        assert_eq!(stepped.position.y, authored_position.y);
        assert_eq!(stepped.position.z, target_ground);
        assert!(!stepped.drawing_disabled());

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.backend().campaign.state_hash(), authored_reset_hash);
        let reset =
            &session.backend().campaign.level2_compatibility().entities()[usize::from(TREE_SLOT)];
        assert_eq!(reset.position, authored_position);
        assert_eq!(reset.action, 0);

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this proves one reset-live Level 3 ordinary Tree placement,
        // map relocation, explicit reopen/reset, and its dry-land ground callback.
        // It does not prove water hiding/splash, damage/death allocation, authored
        // group Trees, other scenery, save/replay, or Unity presentation.
    }

    #[test]
    fn campaign_backend_moves_real_level_three_area_trigger_and_runs_ordinary_group8() {
        const TRIGGER_SLOT: u16 = 224;
        const GROUP8_ROWS: &[u16] = &[208, 209, 210, 211, 212, 213, 214, 215];
        let root = level_three_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let trigger =
            sim_definition::adapter::stable_entity_id("switch.area_entry_trigger.at_218_93")
                .unwrap();
        let package_tile = TilePosition { x: 218, y: 93 };
        let authored_tile = TilePosition { x: 200, y: 80 };
        let package_entity = base.level_definition().unwrap().entity(&trigger).unwrap();
        assert_eq!(package_entity.position(), package_tile);
        assert_eq!(
            package_entity.position_access(),
            LevelEntityPositionAccess::SessionReset
        );

        let workspace_root = temporary_workspace_path("campaign-l3-area-trigger-placement");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-l3-area-trigger-placement",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            75,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let package_position = session.backend().campaign.level2_compatibility().entities()
            [usize::from(TRIGGER_SLOT)]
        .position;

        let change_set = ChangeSetBuilder::new(
            "campaign-l3-area-trigger-placement",
            &base,
            "move-level-three-area-trigger",
            RequestedTiming::SessionReset,
        )
        .move_entity_from(trigger.clone(), package_tile, authored_tile)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();
        assert_eq!(published.revision(), 2);
        assert_eq!(
            published
                .level_definition()
                .unwrap()
                .entity(&trigger)
                .unwrap()
                .position(),
            authored_tile
        );
        assert_eq!(session.identity().definition_hash, base.definition_hash());

        let authored_position = crate::Position {
            x: (authored_tile.x << 8) | (package_position.x & 0xff),
            y: (authored_tile.y << 8) | (package_position.y & 0xff),
            z: package_position.z,
        };
        let player = session
            .backend()
            .campaign
            .level2_compatibility()
            .session
            .state
            .live
            .core
            .state
            .entity_index;
        session
            .backend_mut()
            .campaign
            .level2_compatibility_mut()
            .set_level2_entity_position(player, authored_position)
            .unwrap();
        for _ in 0..8 {
            session
                .step(guard(&session), GameCommand::default())
                .unwrap();
        }
        assert!(session.backend().campaign.events().iter().all(|event| {
            !matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated { group: 8, .. }
            )
        }));
        assert_eq!(
            session.backend().campaign.level2_compatibility().entities()[usize::from(TRIGGER_SLOT)]
                .position,
            package_position,
            "publication must not mutate the already-open package authority"
        );

        let reopened = session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        assert_eq!(reopened.identity.generation, 2);
        assert_eq!(reopened.identity.definition_revision, 2);
        let authored_reset_hash = session.backend().campaign.state_hash();
        {
            let live = &session.backend().campaign.level2.session.state.live;
            assert_eq!(
                live.economy.entities[usize::from(TRIGGER_SLOT)].position,
                authored_position
            );
            assert_eq!(
                live.level2_imported_switch_roots
                    .iter()
                    .find(|root| root.source_slot == TRIGGER_SLOT)
                    .unwrap()
                    .position,
                authored_position
            );
            assert_eq!(
                live.economy.map_chains().tile_of(TRIGGER_SLOT),
                u16::from_le_bytes([authored_tile.x as u8, authored_tile.y as u8])
            );
        }

        session
            .backend_mut()
            .campaign
            .level2_compatibility_mut()
            .set_level2_entity_position(player, package_position)
            .unwrap();
        for _ in 0..8 {
            session
                .step(guard(&session), GameCommand::default())
                .unwrap();
        }
        assert!(session.backend().campaign.events().iter().all(|event| {
            !matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated { group: 8, .. }
            )
        }));

        session
            .backend_mut()
            .campaign
            .level2_compatibility_mut()
            .set_level2_entity_position(player, authored_position)
            .unwrap();
        for _ in 0..8 {
            session
                .step(guard(&session), GameCommand::default())
                .unwrap();
            if session.backend().campaign.events().iter().any(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::AuthoredGroupActivated {
                        source_slot: TRIGGER_SLOT,
                        group: 8,
                        authored_indices,
                        ..
                    } if authored_indices.as_ref() == GROUP8_ROWS
                )
            }) {
                break;
            }
        }
        let compatibility = session.backend().campaign.level2_compatibility();
        assert!(compatibility.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated {
                    source_slot: TRIGGER_SLOT,
                    group: 8,
                    authored_indices,
                    ..
                } if authored_indices.as_ref() == GROUP8_ROWS
            )
        }));
        assert!(
            compatibility
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Level2DispatchEvent::Error { .. }))
        );
        let live = &compatibility.session.state.live;
        assert_eq!(
            live.level2_model39_activated,
            BTreeSet::from([208, 209, 210, 211])
        );
        assert_eq!(
            live.level2_mana_attraction_activated,
            BTreeSet::from([212, 213, 214, 215])
        );
        assert_eq!(compatibility.economy().spheres().len(), 4);
        assert_eq!(
            compatibility
                .economy()
                .mana_attraction_fields()
                .iter()
                .map(|field| field.authored_index)
                .collect::<Vec<_>>(),
            vec![212, 213, 214, 215]
        );
        let fired = &live.economy.entities[usize::from(TRIGGER_SLOT)];
        assert!(fired.active && fired.drawing_disabled());

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.backend().campaign.state_hash(), authored_reset_hash);
        let reset = &session.backend().campaign.level2.session.state.live;
        assert_eq!(
            reset.economy.entities[usize::from(TRIGGER_SLOT)].position,
            authored_position
        );
        assert!(!reset.economy.entities[usize::from(TRIGGER_SLOT)].drawing_disabled());
        assert!(reset.level2_model39_activated.is_empty());
        assert!(reset.level2_mana_attraction_activated.is_empty());

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this proves one reset-imported Level 3 area trigger,
        // explicit reopen/reset, ordinary overlap cadence, and exact group-8
        // model-39/model-54 construction. It does not make the other area
        // triggers movable or prove groups 9/10, dynamic switches, save/replay,
        // or Unity presentation.
    }

    #[test]
    fn campaign_backend_binds_real_experience_scroll_award_before_collection_after_reopen_and_reset()
     {
        const AUTHORED_RECORD: u16 = 248;
        const AUTHORED_AWARD: u32 = 8;

        fn collect_scroll(
            session: &mut GameSession<Mc2CampaignBackend>,
            expected_award: i32,
        ) -> ([i32; 26], [i32; 26]) {
            session
                .backend_mut()
                .campaign
                .level2_compatibility_mut()
                .activate_authored_group_prerequisite_for_test(5)
                .unwrap();
            let scroll = session
                .backend()
                .campaign
                .level2_compatibility()
                .economy()
                .authored_class14()
                .iter()
                .find(|source| source.authored_index == AUTHORED_RECORD)
                .copied()
                .unwrap();
            assert_eq!(scroll.experience_award, Some(expected_award));
            let compatibility = session.backend().campaign.level2_compatibility();
            let before = *compatibility.shared_spells().spell_experience();
            let local_player = compatibility
                .shared_spells()
                .players()
                .iter()
                .find(|player| player.entity_index == compatibility.state.entity_index)
                .unwrap();
            let enabled = local_player.enabled_spell_holders;
            let player_entity = compatibility.state.entity_index;
            session
                .backend_mut()
                .campaign
                .level2_compatibility_mut()
                .set_level2_entity_position(player_entity, scroll.position)
                .unwrap();
            session
                .step(guard(session), GameCommand::default())
                .unwrap();
            let compatibility = session.backend().campaign.level2_compatibility();
            let after = *compatibility.shared_spells().spell_experience();
            for family in 0..after.len() {
                if enabled[family] == 0 {
                    assert_eq!(after[family], before[family]);
                } else if family == usize::from(crate::level1_mission::CASTLE_SPELL_FAMILY) {
                    assert_eq!(
                        after[family],
                        before[family].wrapping_add(expected_award).min(7)
                    );
                } else {
                    assert_eq!(after[family], before[family].wrapping_add(expected_award));
                }
            }
            let retained = compatibility
                .economy()
                .authored_class14()
                .iter()
                .find(|source| source.source_entity_index == scroll.source_entity_index)
                .unwrap();
            assert!(retained.active);
            assert!(!retained.drawing_enabled);
            (before, after)
        }

        let root = descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let scroll =
            sim_definition::adapter::stable_entity_id("item.experience_scroll.at_129_149").unwrap();
        let config = base
            .level_definition()
            .unwrap()
            .entity(&scroll)
            .unwrap()
            .experience_scroll()
            .unwrap();
        assert_eq!(
            (config.default_experience_award(), config.experience_award()),
            (4, 4)
        );

        let workspace_root = temporary_workspace_path("campaign-experience-scroll-award");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-experience-scroll-award",
            base.clone(),
        )
        .unwrap();
        let published = publish_experience_scroll_award(
            &store,
            &catalog,
            &base,
            scroll.clone(),
            AUTHORED_AWARD,
        );
        assert_eq!(
            published
                .level_definition()
                .unwrap()
                .entity(&scroll)
                .unwrap()
                .experience_scroll()
                .unwrap()
                .experience_award(),
            AUTHORED_AWARD
        );

        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            176,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let (_, package_after) = collect_scroll(&mut session, 4);
        assert_eq!(session.identity().definition_hash, base.definition_hash());

        let reopened = session
            .reopen(guard(&session), Arc::new(published.clone()))
            .unwrap();
        assert_eq!(
            (
                reopened.identity.generation,
                reopened.identity.definition_revision
            ),
            (2, 2)
        );
        let authored_frame_zero_hash = session.backend().campaign.state_hash();
        let (_, authored_after) = collect_scroll(&mut session, AUTHORED_AWARD as i32);
        assert_ne!(authored_after, package_after);

        session.reset(guard(&session)).unwrap();
        assert_eq!(
            session.backend().campaign.state_hash(),
            authored_frame_zero_hash
        );
        assert!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .economy()
                .authored_class14()
                .is_empty()
        );
        let (_, reset_after) = collect_scroll(&mut session, AUTHORED_AWARD as i32);
        assert_eq!(reset_after, authored_after);

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this proves per-instance Experience Scroll reward publication,
        // old-session isolation, constructor binding, ordinary local-player XP
        // consumption, hide, explicit reopen, and reset for one real Level 2 row.
        // It does not author placement in the same change, scroll population,
        // enabled spell families, remote-player rewards, save/replay, or Unity UI.
    }

    #[test]
    fn campaign_backend_moves_real_experience_scroll_before_ordinary_collection_and_reclaim() {
        const AUTHORED_RECORD: u16 = 248;
        let root = descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let scroll =
            sim_definition::adapter::stable_entity_id("item.experience_scroll.at_129_149").unwrap();
        let package_tile = TilePosition { x: 129, y: 149 };
        let authored_tile = TilePosition { x: 100, y: 100 };
        let package_entity = base.level_definition().unwrap().entity(&scroll).unwrap();
        assert_eq!(package_entity.position(), package_tile);
        assert_eq!(
            package_entity.position_access(),
            LevelEntityPositionAccess::SessionReset
        );

        let workspace_root = temporary_workspace_path("campaign-experience-scroll-placement");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-experience-scroll-placement",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            76,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let change_set = ChangeSetBuilder::new(
            "campaign-experience-scroll-placement",
            &base,
            "move-level-two-experience-scroll",
            RequestedTiming::SessionReset,
        )
        .move_entity_from(scroll.clone(), package_tile, authored_tile)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();
        assert_eq!(published.revision(), 2);
        assert_eq!(
            published
                .level_definition()
                .unwrap()
                .entity(&scroll)
                .unwrap()
                .position(),
            authored_tile
        );

        session
            .backend_mut()
            .campaign
            .level2_compatibility_mut()
            .activate_authored_group_prerequisite_for_test(5)
            .unwrap();
        let package_scroll = session
            .backend()
            .campaign
            .level2_compatibility()
            .economy()
            .authored_class14()
            .iter()
            .find(|source| source.authored_index == AUTHORED_RECORD)
            .copied()
            .unwrap();
        assert_eq!(
            (
                package_scroll.position.x >> 8,
                package_scroll.position.y >> 8
            ),
            (package_tile.x, package_tile.y)
        );
        let player = session
            .backend()
            .campaign
            .level2_compatibility()
            .state
            .entity_index;
        let authored_position = crate::Position {
            x: authored_tile.x << 8,
            y: authored_tile.y << 8,
            z: package_scroll.position.z,
        };
        session
            .backend_mut()
            .campaign
            .level2_compatibility_mut()
            .set_level2_entity_position(player, authored_position)
            .unwrap();
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        assert!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .shared_spells()
                .pickup_events()
                .iter()
                .all(|event| !matches!(
                    event,
                    Level2SharedSpellEvent::ExperienceScrollCollected { source_slot, .. }
                        if *source_slot == package_scroll.source_entity_index
                ))
        );
        assert_eq!(session.identity().definition_hash, base.definition_hash());

        let reopened = session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        assert_eq!(reopened.identity.generation, 2);
        assert_eq!(reopened.identity.definition_revision, 2);
        let compatibility = session.backend().campaign.level2_compatibility();
        let authored_feature = compatibility
            .mission()
            .authored_features()
            .iter()
            .find(|feature| feature.index == AUTHORED_RECORD)
            .unwrap();
        assert_eq!(
            (authored_feature.tile_x, authored_feature.tile_y),
            (authored_tile.x, authored_tile.y)
        );
        let authored_reset_hash = session.backend().campaign.state_hash();

        session
            .backend_mut()
            .campaign
            .level2_compatibility_mut()
            .activate_authored_group_prerequisite_for_test(5)
            .unwrap();
        let authored_scroll = session
            .backend()
            .campaign
            .level2_compatibility()
            .economy()
            .authored_class14()
            .iter()
            .find(|source| source.authored_index == AUTHORED_RECORD)
            .copied()
            .unwrap();
        assert_eq!(
            (
                authored_scroll.position.x >> 8,
                authored_scroll.position.y >> 8
            ),
            (authored_tile.x, authored_tile.y)
        );
        let experience_before = *session
            .backend()
            .campaign
            .level2_compatibility()
            .shared_spells()
            .spell_experience();
        session
            .backend_mut()
            .campaign
            .level2_compatibility_mut()
            .set_level2_entity_position(player, authored_scroll.position)
            .unwrap();
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        let compatibility = session.backend().campaign.level2_compatibility();
        assert!(
            compatibility
                .shared_spells()
                .pickup_events()
                .iter()
                .any(|event| {
                    matches!(
                        event,
                        Level2SharedSpellEvent::ExperienceScrollCollected {
                            source_slot,
                            player_entity_index,
                        } if *source_slot == authored_scroll.source_entity_index
                            && *player_entity_index == player
                    )
                })
        );
        assert_ne!(
            compatibility.shared_spells().spell_experience(),
            &experience_before
        );
        let retained = compatibility
            .economy()
            .authored_class14()
            .iter()
            .find(|source| source.source_entity_index == authored_scroll.source_entity_index)
            .unwrap();
        assert!(retained.active);
        assert!(!retained.drawing_enabled);
        assert_eq!(retained.phase, authored_scroll.phase.wrapping_add(1));
        assert!(
            compatibility
                .economy()
                .map_chains()
                .is_linked(authored_scroll.source_entity_index)
        );

        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        let compatibility = session.backend().campaign.level2_compatibility();
        assert!(
            !compatibility
                .economy()
                .authored_class14()
                .iter()
                .any(|source| source.source_entity_index == authored_scroll.source_entity_index)
        );
        assert!(
            !compatibility
                .economy()
                .map_chains()
                .is_linked(authored_scroll.source_entity_index)
        );

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.backend().campaign.state_hash(), authored_reset_hash);
        let reset = session.backend().campaign.level2_compatibility();
        assert_eq!(
            reset
                .mission()
                .authored_features()
                .iter()
                .find(|feature| feature.index == AUTHORED_RECORD)
                .map(|feature| (feature.tile_x, feature.tile_y)),
            Some((authored_tile.x, authored_tile.y))
        );
        assert!(reset.economy().authored_class14().is_empty());

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this proves package-bound Experience Scroll placement
        // authoring through explicit reopen/reset, the shared authored-feature
        // constructor, and one ordinary Level 2 XP pickup/hide/reclaim chain.
        // It does not prove organic group-5 activation, every scroll's pickup,
        // spell-experience tuning, other items, save/replay, or Unity rendering.
    }

    #[test]
    fn campaign_backend_moves_real_exit_portal_and_action12_turns_to_authored_target() {
        const PORTAL_RECORD: u16 = 132;
        const TRIGGER_RECORD: u16 = 137;
        let root = level_one_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        let portal_id =
            sim_definition::adapter::stable_entity_id("scenery.exit_portal.at_97_221").unwrap();
        let package_tile = TilePosition { x: 97, y: 221 };
        let authored_tile = TilePosition { x: 75, y: 180 };
        let package_entity = base.level_definition().unwrap().entity(&portal_id).unwrap();
        assert_eq!(package_entity.position(), package_tile);
        assert_eq!(
            package_entity.position_access(),
            LevelEntityPositionAccess::SessionReset
        );

        let workspace_root = temporary_workspace_path("campaign-exit-portal-placement");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-exit-portal-placement",
            base.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            77,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let change_set = ChangeSetBuilder::new(
            "campaign-exit-portal-placement",
            &base,
            "move-level-one-exit-portal",
            RequestedTiming::SessionReset,
        )
        .move_entity_from(portal_id.clone(), package_tile, authored_tile)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();
        assert_eq!(published.revision(), 2);
        assert_eq!(
            published
                .level_definition()
                .unwrap()
                .entity(&portal_id)
                .unwrap()
                .position(),
            authored_tile
        );

        session
            .backend_mut()
            .campaign
            .level2_compatibility_mut()
            .activate_authored_group_prerequisite_for_test(4)
            .unwrap();
        let package_portal = session
            .backend()
            .campaign
            .level2_compatibility()
            .economy()
            .authored_class14()
            .iter()
            .find(|source| source.authored_index == PORTAL_RECORD)
            .copied()
            .unwrap();
        assert_eq!(
            (
                package_portal.position.x >> 8,
                package_portal.position.y >> 8
            ),
            (package_tile.x, package_tile.y),
            "publication must not mutate the already-open package session"
        );
        assert_eq!(session.identity().definition_hash, base.definition_hash());

        let reopened = session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        assert_eq!(reopened.identity.generation, 2);
        assert_eq!(reopened.identity.definition_revision, 2);
        let compatibility = session.backend().campaign.level2_compatibility();
        assert_eq!(
            compatibility
                .mission()
                .authored_features()
                .iter()
                .find(|feature| feature.index == PORTAL_RECORD)
                .map(|feature| (feature.tile_x, feature.tile_y)),
            Some((authored_tile.x, authored_tile.y))
        );
        let authored_reset_hash = session.backend().campaign.state_hash();

        session
            .backend_mut()
            .campaign
            .level2_compatibility_mut()
            .activate_authored_group_prerequisite_for_test(4)
            .unwrap();
        let compatibility = session.backend().campaign.level2_compatibility();
        let authored_portal = compatibility
            .economy()
            .authored_class14()
            .iter()
            .find(|source| source.authored_index == PORTAL_RECORD)
            .copied()
            .unwrap();
        let trigger = compatibility
            .economy()
            .authored_switch_producers()
            .iter()
            .find(|source| source.authored_index == TRIGGER_RECORD)
            .copied()
            .unwrap();
        assert_eq!(
            (
                authored_portal.position.x >> 8,
                authored_portal.position.y >> 8
            ),
            (authored_tile.x, authored_tile.y)
        );
        assert_eq!((authored_portal.model, authored_portal.action), (3, 8));

        let player = compatibility.state.entity_index;
        session
            .backend_mut()
            .campaign
            .level2_compatibility_mut()
            .set_level2_entity_position(player, trigger.position)
            .unwrap();
        let turns_until_cadence = usize::from((8_u8.wrapping_sub(trigger.phase & 7)) & 7) + 1;
        for _ in 0..turns_until_cadence {
            session
                .step(guard(&session), GameCommand::default())
                .unwrap();
        }
        let exit = session
            .backend()
            .campaign
            .level2_compatibility()
            .level_exit_sequence();
        assert!(exit.active);
        assert!(!exit.complete);
        assert_eq!(
            exit.portal_entity_index,
            authored_portal.source_entity_index
        );

        {
            let state = &mut session
                .backend_mut()
                .campaign
                .level2_compatibility_mut()
                .session
                .state
                .live
                .core
                .state;
            state.actual_speed = 0;
        }
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        assert_eq!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .level_exit_sequence()
                .phase,
            1
        );
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        assert_eq!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .level_exit_sequence()
                .phase,
            3
        );

        let player_position = session
            .backend()
            .campaign
            .level2_compatibility()
            .state
            .position;
        let package_target = crate::Position {
            x: package_tile.x << 8,
            y: package_tile.y << 8,
            z: authored_portal.position.z,
        };
        let package_yaw = i16::try_from(crate::heading_for_positions(
            player_position,
            package_target,
        ))
        .unwrap();
        let authored_yaw = i16::try_from(crate::heading_for_positions(
            player_position,
            authored_portal.position,
        ))
        .unwrap();
        assert_ne!(package_yaw, authored_yaw);
        session
            .backend_mut()
            .campaign
            .level2_compatibility_mut()
            .session
            .state
            .live
            .core
            .state
            .yaw = package_yaw;
        let expected_yaw =
            package_yaw.wrapping_add(crate::turn_toward(package_yaw, authored_yaw, 11)) & 0x07ff;
        assert_ne!(expected_yaw, package_yaw);
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        let compatibility = session.backend().campaign.level2_compatibility();
        assert_eq!(compatibility.level_exit_sequence().phase, 3);
        assert_eq!(compatibility.state.yaw, expected_yaw);

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.backend().campaign.state_hash(), authored_reset_hash);
        let reset = session.backend().campaign.level2_compatibility();
        assert_eq!(
            reset.level_exit_sequence(),
            crate::native_world::LevelExitSequence::default()
        );
        assert_eq!(
            reset
                .mission()
                .authored_features()
                .iter()
                .find(|feature| feature.index == PORTAL_RECORD)
                .map(|feature| (feature.tile_x, feature.tile_y)),
            Some((authored_tile.x, authored_tile.y))
        );

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: this proves package-bound placement authoring for all
        // shared Exit Portal carriers through one real Level 1 constructor,
        // capture, and position-sensitive action-12 turn after reopen/reset.
        // It stages group 4 and the player's initial flight pose; it does not
        // prove organic objectives, the full phase-12 transition, L2/L3 exit
        // traversal, save/replay, or Unity presentation.
    }

    #[test]
    fn campaign_backend_binds_exit_sequence_overlap_extent_before_real_trigger_after_reopen_and_reset()
     {
        const TRIGGER_RECORD: u16 = 137;

        fn trigger(
            session: &mut GameSession<Mc2CampaignBackend>,
        ) -> crate::native_world::Level2AuthoredSwitchProducerSource {
            session
                .backend_mut()
                .campaign
                .level2_compatibility_mut()
                .activate_authored_group_prerequisite_for_test(4)
                .unwrap();
            session
                .backend()
                .campaign
                .level2_compatibility()
                .economy()
                .authored_switch_producers()
                .iter()
                .find(|source| source.active && source.authored_index == TRIGGER_RECORD)
                .copied()
                .expect("real group 4 constructs the Exit Sequence trigger")
        }

        fn place_player_three_tiles_away(
            session: &mut GameSession<Mc2CampaignBackend>,
            trigger: crate::native_world::Level2AuthoredSwitchProducerSource,
        ) {
            let player = session
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
                    player,
                    crate::Position {
                        x: trigger.position.x.wrapping_add(3 << 8),
                        ..trigger.position
                    },
                )
                .unwrap();
        }

        fn step_to_trigger_cadence(
            session: &mut GameSession<Mc2CampaignBackend>,
            trigger: crate::native_world::Level2AuthoredSwitchProducerSource,
        ) {
            let turns = usize::from((8_u8.wrapping_sub(trigger.phase & 7)) & 7) + 1;
            for _ in 0..turns {
                session
                    .step(guard(session), GameCommand::default())
                    .unwrap();
            }
        }

        let catalog = default_catalog().unwrap();
        let level_one_root = level_one_descriptor_fixture_path();
        let level_one = load_mc2_campaign(&level_one_root, &catalog).unwrap();
        let level_two = load_mc2_campaign(&descriptor_fixture_path(), &catalog).unwrap();
        let level_three =
            load_mc2_campaign(&level_three_descriptor_fixture_path(), &catalog).unwrap();
        let trigger_ids = [
            "switch.exit_sequence_trigger.at_75_218",
            "switch.exit_sequence_trigger.at_121_155",
            "switch.exit_sequence_trigger.at_213_220",
        ]
        .map(|id| sim_definition::adapter::stable_entity_id(id).unwrap());
        assert_eq!(
            [(&level_one, 0), (&level_two, 1), (&level_three, 2)].map(|(definition, index)| {
                let config = definition
                    .level_definition()
                    .unwrap()
                    .entity(&trigger_ids[index])
                    .unwrap()
                    .switch_overlap()
                    .expect("every package Exit Sequence trigger exposes overlap configuration");
                (
                    config.default_overlap_extent_tiles(),
                    config.overlap_extent_tiles(),
                    config.overlap_extent_tiles_access(),
                )
            }),
            [
                (2, 2, LevelEntityParameterAccess::SessionReset),
                (4, 4, LevelEntityParameterAccess::SessionReset),
                (5, 5, LevelEntityParameterAccess::SessionReset),
            ]
        );

        let workspace_root = temporary_workspace_path("campaign-exit-sequence-overlap");
        let store = WorkspaceStore::create(
            &workspace_root,
            "campaign-exit-sequence-overlap",
            level_one.clone(),
        )
        .unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(level_one.clone()),
            96,
            Mc2CampaignInputs::new(&level_one_root),
        )
        .unwrap();

        let wrong_family =
            sim_definition::adapter::stable_entity_id("switch.area_entry_trigger.at_165_211")
                .unwrap();
        let fake = ChangeSetBuilder::new(
            "campaign-exit-sequence-overlap",
            &level_one,
            "reject-area-entry-overlap-authoring",
            RequestedTiming::SessionReset,
        )
        .set_switch_overlap_extent(wrong_family, 4)
        .build()
        .unwrap();
        let fake_validation = store.validate(&catalog, fake).unwrap();
        assert!(!fake_validation.accepted);

        let change_set = ChangeSetBuilder::new(
            "campaign-exit-sequence-overlap",
            &level_one,
            "set-level-one-exit-sequence-overlap",
            RequestedTiming::SessionReset,
        )
        .set_switch_overlap_extent_from(trigger_ids[0].clone(), 2, 4)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted, "{:#?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = store.active_definition().unwrap();
        assert_eq!(
            published.semantic_target_value(
                &SemanticId::parse("switch.exit_sequence_trigger.at_75_218.overlap_extent_tiles",)
                    .unwrap(),
            ),
            Some(sim_definition::SemanticValue::Integer(4))
        );

        let package_trigger = trigger(&mut session);
        assert_eq!(package_trigger.word_10, 2);
        place_player_three_tiles_away(&mut session, package_trigger);
        step_to_trigger_cadence(&mut session, package_trigger);
        assert_eq!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .level_exit_sequence(),
            crate::native_world::LevelExitSequence::default(),
            "already-open package session retains the two-tile strict overlap"
        );

        session
            .reopen(guard(&session), Arc::new(published))
            .unwrap();
        let authored_reset_hash = session.backend().campaign.state_hash();
        let authored_trigger = trigger(&mut session);
        assert_eq!(authored_trigger.word_10, 4);
        place_player_three_tiles_away(&mut session, authored_trigger);
        step_to_trigger_cadence(&mut session, authored_trigger);
        let compatibility = session.backend().campaign.level2_compatibility();
        assert!(compatibility.level_exit_sequence().active);
        assert!(
            compatibility
                .economy()
                .authored_switch_producers()
                .iter()
                .filter(|source| source.authored_index == TRIGGER_RECORD)
                .all(|source| source.fired && !source.drawing_enabled),
            "successful strict overlap consumes the Exit Sequence trigger"
        );

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.backend().campaign.state_hash(), authored_reset_hash);
        let reset_trigger = trigger(&mut session);
        assert_eq!(reset_trigger.word_10, 4);
        place_player_three_tiles_away(&mut session, reset_trigger);
        step_to_trigger_cadence(&mut session, reset_trigger);
        assert!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .level_exit_sequence()
                .active
        );

        std::fs::remove_dir_all(&workspace_root).unwrap();
        // No-Claim: proves all three package-present Exit Sequence triggers expose their source word-10 overlap extent, and one real Level 1 trigger binds 2-to-4 across guarded publication, old-session isolation, grouped construction, strict overlap, same-pass trigger consumption, LevelExitSequence start, and reset. It does not extend authoring to target group, cadence, portal selection or placement, player bounds, Area Entry/Exit triggers, Level 2/3 traversal, save/replay, or Unity UI.
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

        let published = publish_objective(
            &store,
            &catalog,
            &base,
            "campaign-objective-authoring",
            objective_id,
            original,
            authored.clone(),
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
    fn campaign_backend_castle_mana_thresholds_bind_real_level_one_and_three_after_reopen_and_reset()
     {
        // No-Claim: this focused consumer gate stages the selected objective,
        // group-5 prerequisite, Castle, and mana authority. It does not prove
        // organic preceding-objective progression, Castle construction or mana
        // collection, the sealed legacy Level1Mission authoring route,
        // save/replay, or any other objective-condition family.
        const AUTHORED_THRESHOLD: u16 = 50;
        let catalog = default_catalog().unwrap();
        let cases = [
            (
                level_one_descriptor_fixture_path(),
                "objective.charge.first_castle",
                3_usize,
                15_u16,
                6_u16,
                &[108_u16, 134, 135, 154, 189, 193][..],
            ),
            (
                level_three_descriptor_fixture_path(),
                "objective.charge.reveal_entrance",
                2_usize,
                30_u16,
                2_u16,
                &[146_u16][..],
            ),
        ];

        for (case_index, (root, objective_id, stage, default, group, group_rows)) in
            cases.into_iter().enumerate()
        {
            let base = load_mc2_campaign(&root, &catalog).unwrap();
            assert!(base.required_capabilities().iter().any(|requirement| {
                requirement.id.as_str() == "engine.objective.castle_mana_percent"
                    && requirement.exact_version == 1
            }));
            let workspace_root =
                temporary_workspace_path(&format!("campaign-castle-objective-{case_index}"));
            let workspace = format!("campaign-castle-objective-{case_index}");
            let store = WorkspaceStore::create(&workspace_root, &workspace, base.clone()).unwrap();
            let mut session = GameSession::open(
                &Mc2CampaignAdapter,
                Arc::new(base.clone()),
                80 + case_index as u64,
                Mc2CampaignInputs::new(&root),
            )
            .unwrap();
            let published = publish_objective(
                &store,
                &catalog,
                &base,
                &workspace,
                objective_id,
                ObjectiveCondition::CastleManaPercent(default),
                ObjectiveCondition::CastleManaPercent(AUTHORED_THRESHOLD),
            );

            let (old_percent, objective_index) =
                stage_castle_mana_objective_percent(&mut session, stage, 45);
            assert!((45..i32::from(AUTHORED_THRESHOLD)).contains(&old_percent));
            session
                .step(guard(&session), GameCommand::default())
                .unwrap();
            let compatibility = session.backend().campaign.level2_compatibility();
            let player_entity_index = compatibility
                .session
                .state
                .live
                .core
                .players
                .iter()
                .find(|player| usize::from(player.player_color_index()) == objective_index)
                .map(|player| player.entity_index)
                .expect("the objective owner remains present after the ordinary step");
            let economy = compatibility.economy();
            let castle = economy.castle().unwrap();
            let old_post_step_percent = 100_i32
                * economy
                    .owner_mana_aggregate(player_entity_index)
                    .saturating_add(castle.mana)
                / economy.owner_mana_global();
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
                    .objectives[objective_index]
                    .stage_selectors[stage],
                2,
                "case {case_index} old generation retained {old_post_step_percent}% against package default {default}"
            );

            session
                .reopen(guard(&session), Arc::new(published.clone()))
                .unwrap();
            let objective = sim_definition::adapter::stable_objective_id(objective_id).unwrap();
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
                ObjectiveCondition::CastleManaPercent(AUTHORED_THRESHOLD)
            );
            let (authored_percent, reopened_objective_index) =
                stage_castle_mana_objective_percent(&mut session, stage, 45);
            assert_eq!(reopened_objective_index, objective_index);
            assert!((45..i32::from(AUTHORED_THRESHOLD)).contains(&authored_percent));
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
                    .objectives[objective_index]
                    .stage_selectors[stage],
                1,
                "the authored threshold must replace the raw package default"
            );

            // The ordinary Castle upkeep releases one source-sized reserve
            // before the post-entity objective pass. Stage enough authority
            // for the retained post-upkeep percentage to remain above 50.
            let completed_percent = set_staged_castle_mana_percent(&mut session, 100);
            assert!(completed_percent >= i32::from(AUTHORED_THRESHOLD));
            session
                .step(guard(&session), GameCommand::default())
                .unwrap();
            let compatibility = session.backend().campaign.level2_compatibility();
            let player_entity_index = compatibility
                .session
                .state
                .live
                .core
                .players
                .iter()
                .find(|player| usize::from(player.player_color_index()) == objective_index)
                .map(|player| player.entity_index)
                .expect("the objective owner remains present after the ordinary step");
            let economy = compatibility.economy();
            let castle = economy.castle().unwrap();
            let post_step_percent = 100_i32
                * economy
                    .owner_mana_aggregate(player_entity_index)
                    .saturating_add(castle.mana)
                / economy.owner_mana_global();
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
                    .objectives[objective_index]
                    .stage_selectors[stage],
                2,
                "case {case_index} retained {post_step_percent}% after objective service"
            );
            assert!(
                session
                    .backend()
                    .campaign
                    .events()
                    .iter()
                    .all(|event| !matches!(
                        event,
                        Level2DispatchEvent::AuthoredGroupActivated { group: emitted, .. }
                            if *emitted == group
                    ))
            );
            session
                .step(guard(&session), GameCommand::default())
                .unwrap();
            assert!(
                session
                    .backend()
                    .campaign
                    .events()
                    .iter()
                    .any(|event| matches!(
                        event,
                        Level2DispatchEvent::AuthoredGroupActivated {
                            group: emitted,
                            authored_indices,
                            ..
                        } if *emitted == group && authored_indices.as_ref() == group_rows
                    ))
            );

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
                ObjectiveCondition::CastleManaPercent(AUTHORED_THRESHOLD)
            );
            let (reset_percent, reset_objective_index) =
                stage_castle_mana_objective_percent(&mut session, stage, 45);
            assert_eq!(reset_objective_index, objective_index);
            assert!((45..i32::from(AUTHORED_THRESHOLD)).contains(&reset_percent));
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
                    .objectives[objective_index]
                    .stage_selectors[stage],
                1,
                "reset must retain the authored threshold"
            );

            std::fs::remove_dir_all(workspace_root).unwrap();
        }

        let level_two = load_mc2_campaign(descriptor_fixture_path(), &catalog).unwrap();
        assert!(level_two.required_capabilities().iter().all(|requirement| {
            requirement.id.as_str() != "engine.objective.castle_mana_percent"
        }));
    }

    #[test]
    fn campaign_backend_no_living_creature_retarget_drives_real_level_three_gate_after_reopen_and_reset()
     {
        const OBJECTIVE_ID: &str = "objective.clear.undead_horde";
        const GATE_SLOT: u16 = 127;
        const GROUP_ROWS: [u16; 2] = [141, 142];

        // No-Claim: this focused consumer gate uses the real Level-3 reset
        // census and package objective gate, but it does not prove organic
        // creature deaths, all six absence-objective occurrences, later
        // progression/completion, save/replay, Unity, or cross-family changes.
        let root = level_three_descriptor_fixture_path();
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&root, &catalog).unwrap();
        assert!(base.required_capabilities().iter().any(|requirement| {
            requirement.id.as_str() == "engine.objective.no_living_creatures"
                && requirement.exact_version == 1
        }));
        let workspace_root = temporary_workspace_path("campaign-no-living-objective");
        let workspace = "campaign-no-living-objective";
        let store = WorkspaceStore::create(&workspace_root, workspace, base.clone()).unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(base.clone()),
            91,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let published = publish_objective(
            &store,
            &catalog,
            &base,
            workspace,
            OBJECTIVE_ID,
            ObjectiveCondition::NoLivingCreatures(CreatureKind::Skeleton),
            ObjectiveCondition::NoLivingCreatures(CreatureKind::Dragon),
        );

        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        assert_eq!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .runtime()
                .objectives[0]
                .stage_selectors[0],
            1,
            "the already-open package generation must retain its Skeleton objective"
        );
        assert!(
            session
                .backend()
                .campaign
                .events()
                .iter()
                .all(|event| !matches!(
                    event,
                    Level2DispatchEvent::AuthoredGroupActivated { group: 1, .. }
                ))
        );

        session
            .reopen(guard(&session), Arc::new(published.clone()))
            .unwrap();
        let objective = sim_definition::adapter::stable_objective_id(OBJECTIVE_ID).unwrap();
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
            ObjectiveCondition::NoLivingCreatures(CreatureKind::Dragon)
        );
        let authored_reset_hash = session.backend().campaign.state_hash();

        for pass in 0..2 {
            session
                .step(guard(&session), GameCommand::default())
                .unwrap();
            if pass == 0 {
                assert_eq!(
                    session
                        .backend()
                        .campaign
                        .level2_compatibility()
                        .runtime()
                        .objectives[0]
                        .stage_selectors[0],
                    2
                );
                assert!(
                    session
                        .backend()
                        .campaign
                        .events()
                        .iter()
                        .all(|event| !matches!(
                            event,
                            Level2DispatchEvent::AuthoredGroupActivated { group: 1, .. }
                        ))
                );
            }
        }
        assert!(
            session
                .backend()
                .campaign
                .events()
                .iter()
                .any(|event| matches!(
                    event,
                    Level2DispatchEvent::AuthoredGroupActivated {
                        source_slot: GATE_SLOT,
                        group: 1,
                        authored_indices,
                        ..
                    } if authored_indices.as_ref() == GROUP_ROWS
                ))
        );
        assert_eq!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .runtime()
                .objectives[0]
                .stage_selectors[1],
            1,
            "the objective-done pause must defer the next semantic predicate"
        );

        session.reset(guard(&session)).unwrap();
        assert_eq!(session.backend().campaign.state_hash(), authored_reset_hash);
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
            ObjectiveCondition::NoLivingCreatures(CreatureKind::Dragon)
        );
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        assert_eq!(
            session
                .backend()
                .campaign
                .level2_compatibility()
                .runtime()
                .objectives[0]
                .stage_selectors[0],
            2
        );
        assert!(
            session
                .backend()
                .campaign
                .events()
                .iter()
                .all(|event| !matches!(
                    event,
                    Level2DispatchEvent::AuthoredGroupActivated { group: 1, .. }
                ))
        );
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        assert!(
            session
                .backend()
                .campaign
                .events()
                .iter()
                .any(|event| matches!(
                    event,
                    Level2DispatchEvent::AuthoredGroupActivated {
                        source_slot: GATE_SLOT,
                        group: 1,
                        authored_indices,
                        ..
                    } if authored_indices.as_ref() == GROUP_ROWS
                ))
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
                    .join("../sim-definition/tests/fixtures/custom-synthetic-v1"),
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
