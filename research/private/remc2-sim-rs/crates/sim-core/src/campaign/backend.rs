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
        FIREBALL_MANA_COST_CAPABILITY_ID, GameDefinition, RequiredCapability, SemanticCatalog,
        SemanticId, default_catalog, engine_capabilities,
    },
    game_session::{
        CampaignSnapshot, EventPage, GameAdapter, GameAssetReference, GameAudioCommand,
        GameCommand, GameCompletion, GameCompletionStatus, GameEvent, GameSessionBackend,
        LateralControl, LongitudinalControl, PlayerMotion, PlayerPose, PresentationEntity,
        PresentationPose, RuntimeActorId, RuntimeParticipantId,
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
        Access, ArtifactId, ChangeSetBuilder, GamePackage, IntegerParameter, OptionParameter,
        PresentationAssetSlot, PublicationFault, RequestedTiming, SemanticValue, WorkspaceStore,
    };
    use crate::game_session::{GameSession, SessionGuard};
    use crate::goat::{GoatBehaviorConfig, GoatStepEvent};
    use crate::level_definition::{
        ActivationGroupId, BuildingKind, CreatureKind, ItemKind, LevelEntityKind,
        LevelEntityParameterAccess, LevelEntityPositionAccess, LevelObjectiveConditionAccess,
        MechanicKind, ObjectiveCondition, SceneryKind, SpellKind, SwitchKind, TilePosition,
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
        assert_eq!(
            level_three
                .level_definition()
                .unwrap()
                .entities()
                .iter()
                .filter(|entity| {
                    entity.position_access() == LevelEntityPositionAccess::SessionReset
                })
                .map(|entity| entity.id().as_str())
                .collect::<Vec<_>>(),
            vec!["spell_pickup.shield.at_85_85"]
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
