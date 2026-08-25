//! Descriptor-backed campaign session facade.
//!
//! The sealed Level 1 route and legacy Level 2 inspection API remain
//! compatibility adapters. New campaign sessions validate a complete
//! [`spec::CampaignDefinition`] before any mutable inspection/session state is
//! constructed, then delegate the already-qualified Level 2 behavior to the
//! folded Rust authority.

use std::{fmt, path::Path, sync::Arc};

use crate::{
    PlayerCommand, StepEvidence,
    native_world::{Level2DispatchEvent, NativeWorld, NativeWorldError, NativeWorldInspection},
};

pub mod backend;
pub mod dispatch;
pub mod journal;
pub mod objectives;
pub mod schedule;
#[doc(hidden)]
pub mod source_arena;
pub mod source_facts;
pub mod spec;
pub mod state;

pub use crate::native_world::Level2ResetSession;

use dispatch::CAMPAIGN_BEHAVIOR_REGISTRY;
use journal::{EffectCursor, EffectPage, EffectPageError};
use spec::{CampaignDefinition, CampaignPackageError, CampaignSelection};

/// Failure to validate or materialize a campaign session. A validation error
/// is always returned before an inspection, reset state, or live session has
/// been constructed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CampaignOpenError {
    Preflight(CampaignPackageError),
    Materialization(NativeWorldError),
}

impl fmt::Display for CampaignOpenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Preflight(error) => write!(formatter, "{error}"),
            Self::Materialization(error) => write!(formatter, "campaign materialization: {error}"),
        }
    }
}

impl std::error::Error for CampaignOpenError {}

/// Neutral owner for a validated campaign definition and its one live Rust
/// authority. The concrete Level 2 adapter stays private to this fold leaf;
/// CF-12 projects the common operations without adding a per-level ABI.
#[derive(Clone, Debug)]
pub struct CampaignSession {
    definition: Arc<CampaignDefinition>,
    inspection: Arc<NativeWorldInspection>,
    level2: Level2ResetSession,
}

impl CampaignSession {
    /// Validate and open a campaign package. Contract v1 admits the
    /// census-profiled selections (qualified Level 2 and Level 3); unsupported
    /// identities fail during descriptor preflight rather than selecting a
    /// fallback implementation.  A profiled selection whose census-delta
    /// shared-behavior leaves are not yet compiled (Level 3: ClassAction 2/21,
    /// 11/16, 14/6 per mc-iol7n) fails closed at behavior closure before this
    /// materializer runs, so the folded shared authority below stays the one
    /// live engine and no per-level route exists.
    pub fn open(
        package_root: impl AsRef<Path>,
        selection: CampaignSelection,
    ) -> Result<Self, CampaignOpenError> {
        Self::open_with_materializer(package_root.as_ref(), selection, |definition| {
            let inspection = NativeWorld::inspect_campaign_level2(definition)?;
            let level2 = inspection.level2_reset_session_from_definition(definition)?;
            Ok((inspection, level2))
        })
    }

    /// Open with an explicit private gameplay cache while keeping owner bytes
    /// outside the package and distributable client tree.
    pub fn open_with_owner_cache(
        package_root: impl AsRef<Path>,
        owner_cache: impl AsRef<Path>,
        selection: CampaignSelection,
    ) -> Result<Self, CampaignOpenError> {
        let owner_cache = owner_cache.as_ref();
        Self::open_with_materializer(package_root.as_ref(), selection, |definition| {
            let inspection =
                NativeWorld::inspect_campaign_level2_with_owner_cache(definition, owner_cache)?;
            let level2 = inspection.level2_reset_session_from_definition(definition)?;
            Ok((inspection, level2))
        })
    }

    fn open_with_materializer(
        package_root: &Path,
        selection: CampaignSelection,
        materialize: impl FnOnce(
            &CampaignDefinition,
        ) -> Result<
            (NativeWorldInspection, Level2ResetSession),
            NativeWorldError,
        >,
    ) -> Result<Self, CampaignOpenError> {
        let definition =
            CampaignDefinition::preflight(package_root, selection, &CAMPAIGN_BEHAVIOR_REGISTRY)
                .map_err(CampaignOpenError::Preflight)?;
        let (inspection, level2) =
            materialize(&definition).map_err(CampaignOpenError::Materialization)?;
        Ok(Self {
            definition: Arc::new(definition),
            inspection: Arc::new(inspection),
            level2,
        })
    }

    #[must_use]
    pub fn definition(&self) -> &CampaignDefinition {
        &self.definition
    }

    #[must_use]
    pub fn inspection(&self) -> &NativeWorldInspection {
        &self.inspection
    }

    /// Step the one authoritative Rust state through the source scheduler.
    pub fn step(&mut self, command: PlayerCommand, speed_index: u8) -> Vec<StepEvidence> {
        self.level2.step_outer_turn(command, speed_index)
    }

    pub fn reset(&mut self) {
        self.level2.reset();
    }

    /// Current runtime generation of the one folded session authority.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.level2.session.generation
    }

    /// Current source tick of the one folded session authority.
    #[must_use]
    pub fn tick(&self) -> u64 {
        self.level2.state.tick
    }

    /// Cursor immediately after the last retained production effect.
    #[must_use]
    pub fn effect_cursor(&self) -> EffectCursor {
        self.level2.effect_journal().cursor(self.generation())
    }

    /// Page the canonical ordered production journal without exposing a
    /// mutable journal or any Level-2-specific event sidecar.
    pub fn effects_after(
        &self,
        cursor: EffectCursor,
        capacity: usize,
    ) -> Result<EffectPage, EffectPageError> {
        self.level2
            .effect_journal()
            .page(self.generation(), cursor, capacity)
    }

    #[must_use]
    pub fn state_hash(&self) -> u64 {
        self.level2.campaign_state_hash()
    }

    #[must_use]
    pub fn terrain_hash(&self) -> u64 {
        self.level2.terrain_hash()
    }

    #[must_use]
    pub fn events(&self) -> &[Level2DispatchEvent] {
        self.level2.dispatch_events()
    }

    /// Borrow the folded Level 2 state for the already-shipped named ABI
    /// projections. This is a compatibility view of this session's sole live
    /// authority, not a second session or a level-selection API. Generic
    /// clients must use the neutral lifecycle and journal/frame projections.
    #[doc(hidden)]
    #[must_use]
    pub fn level2_compatibility(&self) -> &Level2ResetSession {
        &self.level2
    }

    /// Mutable counterpart used only by the existing Level 2 ABI adapters.
    /// All mutations still target this session's one authoritative state.
    #[doc(hidden)]
    pub fn level2_compatibility_mut(&mut self) -> &mut Level2ResetSession {
        &mut self.level2
    }
}

#[cfg(test)]
mod tests {
    use super::{CampaignOpenError, CampaignSelection, CampaignSession, Level2ResetSession};
    use crate::{
        PlayerCommand,
        campaign::{
            dispatch::CAMPAIGN_BEHAVIOR_REGISTRY,
            spec::{BehaviorSelector, CampaignDefinition, ValidationStage},
        },
        native_world::{Level2NyphurDispatchCall, NativeWorld},
    };
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicBool, Ordering},
    };

    const OMEGA: &str =
        include_str!("../../../../level2-conformance/fold/campaign-fold-omega-v1.1.json");

    fn level_two_fixture_path() -> PathBuf {
        std::env::var_os("MC_LEVEL2_PACKAGE_V5").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/l2-debug3.fJ3PaM/level2")
            },
            PathBuf::from,
        )
    }

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

    #[test]
    fn cf11_level2_opens_steps_resets_through_descriptor() {
        let legacy_inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("legacy Level 2 compatibility package loads");
        let mut legacy = legacy_inspection
            .level2_reset_session()
            .expect("legacy Level 2 compatibility session opens");
        let mut campaign =
            CampaignSession::open(descriptor_fixture_path(), CampaignSelection::LEVEL2)
                .expect("qualified campaign definition opens one Rust session");

        assert_eq!(
            campaign.definition().completed_stage,
            ValidationStage::ResetState
        );
        assert_eq!(campaign.definition().identity, CampaignSelection::LEVEL2);
        assert_ne!(
            campaign.definition().package_sha256,
            campaign.inspection().package_sha256(),
            "campaign envelope and wrapped owner export have distinct identities"
        );
        assert_eq!(
            campaign.inspection().package_sha256(),
            legacy_inspection.package_sha256(),
            "materialized world and owner cache bind to the wrapped export identity"
        );
        assert_eq!(campaign.state_hash(), legacy.campaign_state_hash());
        assert_eq!(campaign.terrain_hash(), legacy.terrain_hash());
        let initial_state = campaign.state_hash();
        let initial_terrain = campaign.terrain_hash();

        let command = PlayerCommand::default();
        let descriptor_evidence = campaign.step(command, 0);
        let legacy_evidence = legacy.step_outer_turn(command, 0);
        assert_eq!(descriptor_evidence, legacy_evidence);
        assert_eq!(campaign.events(), legacy.dispatch_events());
        assert_eq!(campaign.state_hash(), legacy.campaign_state_hash());
        assert_eq!(campaign.terrain_hash(), legacy.terrain_hash());
        let stepped_state = campaign.state_hash();
        let stepped_terrain = campaign.terrain_hash();
        let stepped_events = campaign.events().to_vec();

        campaign.reset();
        assert_eq!(campaign.state_hash(), initial_state);
        assert_eq!(campaign.terrain_hash(), initial_terrain);
        assert!(campaign.events().is_empty());

        let replay_evidence = campaign.step(command, 0);
        assert_eq!(replay_evidence, descriptor_evidence);
        assert_eq!(campaign.events(), stepped_events.as_slice());
        assert_eq!(campaign.state_hash(), stepped_state);
        assert_eq!(campaign.terrain_hash(), stepped_terrain);
    }

    #[test]
    fn cf11_descriptor_preflight_fails_before_session() {
        static MATERIALIZER_CALLED: AtomicBool = AtomicBool::new(false);
        MATERIALIZER_CALLED.store(false, Ordering::SeqCst);
        let mismatched_selection = CampaignSelection {
            external_level: 1,
            internal_level: 0,
            map_type_id: 1,
        };
        let result = CampaignSession::open_with_materializer(
            &descriptor_fixture_path(),
            mismatched_selection,
            |_| {
                MATERIALIZER_CALLED.store(true, Ordering::SeqCst);
                panic!("materializer must not run after failed descriptor preflight")
            },
        );
        let CampaignOpenError::Preflight(error) = result.expect_err("identity mismatch must fail")
        else {
            panic!("identity mismatch escaped descriptor preflight");
        };
        assert_eq!(error.stage, ValidationStage::OuterManifest);
        assert!(!MATERIALIZER_CALLED.load(Ordering::SeqCst));
    }

    /// mc-iol7n census receipt: the fifteen live Level 3 reset selectors.
    const CF14_LEVEL3_LIVE_PAIRS: [(u8, u8); 15] = [
        (2, 0),
        (2, 3),
        (2, 21),
        (3, 0),
        (5, 31),
        (5, 72),
        (5, 232),
        (10, 52),
        (11, 0),
        (11, 4),
        (11, 16),
        (11, 32),
        (14, 6),
        (15, 19),
        (15, 34),
    ];

    /// mc-iol7n census delta: live Level 3 selectors whose shared-behavior
    /// leaves have not landed.  Each is a separate blocking leaf; CF-14 owns
    /// only the descriptor/composition route and must fail closed on them.
    const CF14_LEVEL3_MISSING_PAIRS: [(u8, u8); 3] = [(2, 21), (11, 16), (14, 6)];

    #[test]
    fn cf14_level3_descriptor_uses_shared_engine() {
        let package = level_three_descriptor_fixture_path();

        // The one shared descriptor engine validates the qualified owner
        // Level 3 package through every data stage (identity, members, typed
        // members, source facts, operations, reset receipt) with no per-level
        // route.  Only the compiled-capability gate is exercised separately
        // below, so this receipt binds stages 1-7 and 9 to the census facts.
        let accept = |_: &super::spec::BehaviorKey| true;
        let definition =
            CampaignDefinition::preflight(&package, CampaignSelection::LEVEL3, &accept)
                .expect("census-profiled Level 3 descriptor validates through the shared engine");
        assert_eq!(definition.completed_stage, ValidationStage::ResetState);
        assert_eq!(definition.identity, CampaignSelection::LEVEL3);
        assert!(definition.reset_state_receipt.structurally_validated);
        assert_eq!(definition.source_facts.len(), 3);
        assert_eq!(definition.campaign_spec.objectives.len(), 5);
        assert_eq!(
            definition
                .campaign_spec
                .reset_sources
                .feature_boundary
                .len(),
            7
        );
        assert!(definition.campaign_spec.initial_activation.is_empty());
        assert_eq!(
            definition.campaign_spec.presentation_recipe.map_type, "Day",
            "Level 3 reuses the shared Day presentation facts"
        );
        let live_pairs = definition
            .required_behavior_keys()
            .map(|key| match key.selector {
                BehaviorSelector::ClassAction { class, action } => (class, action),
                ref other => panic!("live root uses non-ClassAction selector {other:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            live_pairs,
            CF14_LEVEL3_LIVE_PAIRS.to_vec(),
            "descriptor live roots must equal the census receipt"
        );
        let missing = definition
            .required_behavior_keys()
            .filter(|key| CAMPAIGN_BEHAVIOR_REGISTRY.entry(key).is_none())
            .map(|key| match key.selector {
                BehaviorSelector::ClassAction { class, action } => (class, action),
                ref other => panic!("missing root uses non-ClassAction selector {other:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            missing,
            CF14_LEVEL3_MISSING_PAIRS.to_vec(),
            "exactly the census-delta shared-behavior leaves stay uncompiled"
        );

        // The production route consumes the same compiled registry and must
        // fail closed at behavior closure, before any materialization, session
        // state, or handle exists.
        static MATERIALIZER_CALLED: AtomicBool = AtomicBool::new(false);
        MATERIALIZER_CALLED.store(false, Ordering::SeqCst);
        let result =
            CampaignSession::open_with_materializer(&package, CampaignSelection::LEVEL3, |_| {
                MATERIALIZER_CALLED.store(true, Ordering::SeqCst);
                panic!("materializer must not run while census-delta leaves are uncompiled")
            });
        let CampaignOpenError::Preflight(error) =
            result.expect_err("Level 3 open fails while census-delta leaves are uncompiled")
        else {
            panic!("Level 3 open escaped descriptor preflight");
        };
        assert_eq!(error.stage, ValidationStage::BehaviorClosure);
        assert!(
            error
                .message
                .contains("compiled registry is missing capability"),
            "closure failure must name the compiled-capability gate: {}",
            error.message
        );
        assert!(
            error.message.contains("class: 2") && error.message.contains("action: 21"),
            "closure failure must name the first census-delta capability: {}",
            error.message
        );
        assert!(!MATERIALIZER_CALLED.load(Ordering::SeqCst));

        // The public route fails with the identical fail-closed receipt.
        let public_error = CampaignSession::open(&package, CampaignSelection::LEVEL3)
            .expect_err("public Level 3 open fails identically");
        assert_eq!(public_error, CampaignOpenError::Preflight(error));

        // Selection mismatch fails at the outer manifest before member decode.
        let mismatch = CampaignDefinition::preflight(&package, CampaignSelection::LEVEL2, &accept)
            .expect_err("Level 2 caller against the Level 3 package must fail");
        assert_eq!(mismatch.stage, ValidationStage::OuterManifest);

        // An unprofiled selection can never borrow the Level 3 route.
        let unprofiled = CampaignSelection {
            external_level: 4,
            internal_level: 3,
            map_type_id: 0,
        };
        let unknown = CampaignDefinition::preflight(&package, unprofiled, &accept)
            .expect_err("an unprofiled selection must fail closed");
        assert_eq!(unknown.stage, ValidationStage::OuterManifest);

        // A mutated member fails the hash stage before any typed decode.
        let tampered_root = std::env::temp_dir().join(format!(
            "mc-cf14-tamper-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&tampered_root).expect("create tamper scratch");
        for entry in std::fs::read_dir(&package).expect("enumerate Level 3 package") {
            let entry = entry.expect("inspect Level 3 package entry");
            std::fs::copy(entry.path(), tampered_root.join(entry.file_name()))
                .expect("copy Level 3 member into tamper scratch");
        }
        let terrain_path = tampered_root.join("terrain.bin");
        let mut terrain = std::fs::read(&terrain_path).expect("read tamper terrain");
        terrain[0] ^= 0x01;
        std::fs::write(&terrain_path, terrain).expect("write tampered terrain");
        let tampered =
            CampaignDefinition::preflight(&tampered_root, CampaignSelection::LEVEL3, &accept)
                .expect_err("a mutated member must fail the hash stage");
        assert_eq!(tampered.stage, ValidationStage::MemberHashes);
        std::fs::remove_dir_all(&tampered_root).expect("remove tamper scratch");
    }

    #[test]
    fn cf01_pure_extraction_preserves_omega() {
        assert!(OMEGA.contains("\"schema_version\": \"campaign-fold-omega-v1.1\""));
        assert!(OMEGA.contains("\"bead_id\": \"mc-cf00-omega-reseal-3dxle\""));
        assert!(OMEGA.contains("\"sealed_from_bead\": \"mc-cf00-seal-fold-inputs-omega-iplbq\""));
        assert!(OMEGA.contains("\"current_open_question_count\": 229"));
        assert!(OMEGA.contains("\"unmapped_open_question_count\": 0"));
        assert!(OMEGA.contains("\"json_byte_identical\": true"));

        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 Omega fixture loads");
        let mut session: Level2ResetSession = inspection
            .level2_reset_session()
            .expect("qualified Level 2 Omega session opens");
        let initial_hash = session.campaign_state_hash();
        let initial_terrain_hash = session.terrain_hash();

        let first_evidence = session.step_outer_turn(PlayerCommand::default(), 0);
        let first_events = session.dispatch_events().to_vec();
        assert!(
            !first_evidence.is_empty(),
            "Omega route must commit one step"
        );
        assert!(
            !first_events.is_empty(),
            "Omega route must emit dispatch rows"
        );
        let first_passes = first_events
            .iter()
            .map(|event| event.campaign_projection().0)
            .collect::<Vec<_>>();
        assert!(
            first_passes.windows(2).all(|window| window[0] <= window[1]),
            "prepass/source/epilogue rows escaped their source-pass order"
        );
        let nyphur_trace = session
            .nyphur_dispatch_traces()
            .first()
            .expect("Omega route emits the ordinary Nyphur source trace");
        assert_eq!(
            (
                nyphur_trace.pass,
                nyphur_trace.source_slot,
                nyphur_trace.state_byte
            ),
            (0, 112, 0),
            "load/prepass must reach the qualified source row before projection"
        );
        assert_eq!(
            nyphur_trace.calls.as_ref(),
            &[
                Level2NyphurDispatchCall::Sub12a70,
                Level2NyphurDispatchCall::Sub12e70,
                Level2NyphurDispatchCall::Sub12e70,
            ],
            "source callback, same-pass epilogue, and commit tail order changed"
        );
        let stepped_hash = session.campaign_state_hash();
        let stepped_terrain_hash = session.terrain_hash();

        session.reset();
        assert_eq!(session.campaign_state_hash(), initial_hash);
        assert_eq!(session.terrain_hash(), initial_terrain_hash);
        assert!(
            session.dispatch_events().is_empty(),
            "reset must clear the committed dispatch projection before replay"
        );

        let replay_evidence = session.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(replay_evidence, first_evidence);
        assert_eq!(session.dispatch_events(), first_events.as_slice());
        assert_eq!(session.campaign_state_hash(), stepped_hash);
        assert_eq!(session.terrain_hash(), stepped_terrain_hash);
    }
}
