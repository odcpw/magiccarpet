//! Descriptor-backed campaign session facade.
//!
//! The sealed Level 1 route and legacy Level 2 inspection API remain
//! compatibility adapters. New campaign sessions validate a complete
//! [`spec::CampaignDefinition`] before any mutable inspection/session state is
//! constructed, then delegate the package-present behavior to the shared Rust
//! authority.

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
/// authority. The retained `Level2ResetSession` name is a private compatibility
/// detail around the shared descriptor-backed engine; CF-12 projects the
/// common operations without adding a per-level ABI.
#[derive(Clone, Debug)]
pub struct CampaignSession {
    definition: Arc<CampaignDefinition>,
    inspection: Arc<NativeWorldInspection>,
    level2: Level2ResetSession,
}

impl CampaignSession {
    /// Validate and open a campaign package. Contract v1 admits the
    /// census-profiled selections (qualified Levels 1, 2, and 3); unsupported
    /// identities fail during descriptor preflight rather than selecting a
    /// fallback implementation. Behavior closure validates every live selector
    /// against the shared compiled registry before this materializer runs, so
    /// the folded authority below stays the one live engine and no per-level
    /// route exists.
    pub fn open(
        package_root: impl AsRef<Path>,
        selection: CampaignSelection,
    ) -> Result<Self, CampaignOpenError> {
        Self::open_with_materializer(package_root.as_ref(), selection, |definition| {
            let inspection = NativeWorld::inspect_campaign_definition(definition)?;
            let level2 = inspection.campaign_reset_session_from_definition(definition)?;
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
                NativeWorld::inspect_campaign_definition_with_owner_cache(definition, owner_cache)?;
            let level2 = inspection.campaign_reset_session_from_definition(definition)?;
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
        FlightEvent, FlightWorld, PlayerCommand,
        campaign::{
            dispatch::CAMPAIGN_BEHAVIOR_REGISTRY,
            schedule::{ScheduleReceipt, SourcePass, VisitKind},
            spec::{BehaviorSelector, CampaignDefinition, ValidationStage},
        },
        native_world::{
            EntitySpecialRelation, Level2DispatchEvent, Level2NyphurDispatchCall, NativeWorld,
        },
    };
    use std::{
        collections::BTreeMap,
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

    fn level_one_descriptor_fixture_path() -> PathBuf {
        std::env::var_os("MC_CAMPAIGN_LEVEL1_PACKAGE_V1").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf15-level1-package-current")
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

    #[test]
    fn cf15_level1_real_reset_families_enter_shared_scheduler() {
        let package = level_one_descriptor_fixture_path();
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL1)
            .expect("real Level 1 descriptor opens through the shared engine");
        let session = campaign.level2_compatibility();

        let raw_slots = |class: u8, model: u8, action: u8| {
            session
                .entities()
                .iter()
                .filter(|row| {
                    row.active && row.class == class && row.model == model && row.action == action
                })
                .map(|row| row.index)
                .collect::<Vec<_>>()
        };
        let goat_slots = raw_slots(5, 1, 15);
        let villager_slots = raw_slots(5, 13, 111);
        let dolmen_slots = raw_slots(2, 2, 6);
        let mut holder_slots = raw_slots(15, 0, 0);
        holder_slots.extend(raw_slots(15, 1, 3));
        holder_slots.sort_unstable();

        assert_eq!(goat_slots.len(), 33);
        assert_eq!(villager_slots.len(), 28);
        assert_eq!(dolmen_slots.len(), 3);
        assert_eq!(holder_slots.len(), 2);
        assert_eq!(session.goats().len(), goat_slots.len());
        assert_eq!(session.ordinary().villagers().len(), villager_slots.len());
        assert_eq!(session.ordinary().dolmens().len(), dolmen_slots.len());
        assert_eq!(session.shared_spells().holders().len(), holder_slots.len());
        assert_eq!(
            session
                .goats()
                .iter()
                .map(|row| row.entity_index)
                .collect::<Vec<_>>(),
            goat_slots
        );
        assert_eq!(
            session
                .ordinary()
                .villagers()
                .iter()
                .map(|row| row.entity_index)
                .collect::<Vec<_>>(),
            villager_slots
        );
        assert_eq!(
            session
                .shared_spells()
                .holders()
                .iter()
                .map(|row| row.index)
                .collect::<Vec<_>>(),
            holder_slots
        );

        let initial_hash = campaign.state_hash();
        let goat_phases = session
            .goats()
            .iter()
            .map(|row| (row.entity_index, row.phase))
            .collect::<Vec<_>>();
        let villager_phases = session
            .ordinary()
            .villagers()
            .iter()
            .map(|row| (row.entity_index, row.phase))
            .collect::<Vec<_>>();
        let holder_phases = session
            .shared_spells()
            .holders()
            .iter()
            .map(|row| (row.index, row.phase))
            .collect::<Vec<_>>();
        let mut hydrated_slots = goat_slots;
        hydrated_slots.extend(villager_slots);
        hydrated_slots.extend(dolmen_slots);
        hydrated_slots.extend(holder_slots);
        hydrated_slots.sort_unstable();

        let _ = campaign.step(PlayerCommand::default(), 0);
        let receipts = campaign
            .level2_compatibility()
            .cf10_production_schedule_receipts();
        for source_slot in hydrated_slots {
            assert!(
                receipts.iter().any(|receipt| matches!(
                    receipt,
                    ScheduleReceipt::SlotResolved {
                        pass: SourcePass::UpdateEntities,
                        scan: 0,
                        slot,
                        kind: VisitKind::Callback,
                        ..
                    } if *slot == source_slot
                )),
                "hydrated Level 1 slot {source_slot} was not resolved as a callback"
            );
            assert!(
                receipts.iter().any(|receipt| matches!(
                    receipt,
                    ScheduleReceipt::CallbackCompleted {
                        pass: SourcePass::UpdateEntities,
                        scan: 0,
                        slot,
                        ..
                    } if *slot == source_slot
                )),
                "hydrated Level 1 slot {source_slot} did not complete its callback"
            );
            assert!(
                receipts.iter().any(|receipt| matches!(
                    receipt,
                    ScheduleReceipt::EpilogueCompleted {
                        pass: SourcePass::UpdateEntities,
                        scan: 0,
                        slot,
                        ..
                    } if *slot == source_slot
                )),
                "hydrated Level 1 slot {source_slot} did not complete its phase epilogue"
            );
            assert!(
                campaign.events().iter().all(|event| !matches!(
                    event,
                    Level2DispatchEvent::Error { source_slot: slot, .. } if *slot == source_slot
                )),
                "hydrated Level 1 slot {source_slot} emitted a dispatch error"
            );
        }
        let session = campaign.level2_compatibility();
        for (slot, phase) in goat_phases {
            assert_eq!(
                session
                    .goats()
                    .iter()
                    .find(|row| row.entity_index == slot)
                    .expect("Goat remains live after one ordinary pass")
                    .phase,
                phase.wrapping_add(1)
            );
        }
        for (slot, phase) in villager_phases {
            assert_eq!(
                session
                    .ordinary()
                    .villagers()
                    .iter()
                    .find(|row| row.entity_index == slot)
                    .expect("Villager remains live after one ordinary pass")
                    .phase,
                phase.wrapping_add(1)
            );
        }
        for (slot, phase) in holder_phases {
            let holder = session
                .shared_spells()
                .holders()
                .iter()
                .find(|row| row.index == slot)
                .expect("initial spell holder remains live after one ordinary pass");
            assert_eq!(holder.phase, phase.wrapping_add(1));
            assert_eq!(session.entities()[usize::from(slot)].phase, holder.phase);
        }

        campaign.reset();
        assert_eq!(campaign.state_hash(), initial_hash);
        assert_eq!(campaign.level2_compatibility().goats().len(), 33);
        assert_eq!(
            campaign.level2_compatibility().ordinary().villagers().len(),
            28
        );
        assert_eq!(
            campaign.level2_compatibility().ordinary().dolmens().len(),
            3
        );
        assert_eq!(
            campaign
                .level2_compatibility()
                .shared_spells()
                .holders()
                .len(),
            2
        );
    }

    #[test]
    fn cf15_level1_group1_materializes_castle_pickup_switches_and_trees_in_source_order() {
        const GROUP_ROWS: [u16; 21] = [
            31, 99, 104, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 168, 169, 170, 171, 172,
            173, 174, 175,
        ];
        const TREE_ROWS: [u16; 18] = [
            155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 168, 169, 170, 171, 172, 173, 174,
            175,
        ];
        const PRODUCER_SLOT: u16 = 48;
        const SPELL_SLOT: u16 = 178;
        const FIRST_TREE_SLOT: u16 = 181;

        let package = level_one_descriptor_fixture_path();
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL1)
            .expect("real Level 1 descriptor opens through the shared engine");
        let initial_hash = campaign.state_hash();
        let session = campaign.level2_compatibility();
        let producer = &session.entities()[usize::from(PRODUCER_SLOT)];
        assert_eq!(
            (
                producer.active,
                producer.class,
                producer.model,
                producer.action,
                producer.source_id,
            ),
            (true, 11, 1, 1, 1),
            "the real package must drive group 1 through its ordinary switch producer"
        );
        let producer_feature = session
            .mission()
            .authored_features()
            .iter()
            .find(|row| row.index == 2)
            .expect("package row 2 is the reset producer source");
        assert_eq!(
            (
                producer_feature.kind,
                producer_feature.subtype,
                producer_feature.activation_group,
                producer_feature.stage_tag,
                producer_feature.word_10,
            ),
            (11, 1, 0, 1, 64)
        );
        let producer_position = producer.position;
        let producer_half_width = i16::from_le_bytes(
            producer.record_bytes()[88..90]
                .try_into()
                .expect("producer render half-width extent"),
        );
        let producer_half_depth = i16::from_le_bytes(
            producer.record_bytes()[90..92]
                .try_into()
                .expect("producer render half-depth extent"),
        );
        assert_eq!((producer_half_width, producer_half_depth), (16_384, 16_384));
        let package_group_rows = session
            .mission()
            .authored_features()
            .iter()
            .filter(|row| row.activation_group == 1 && row.kind != 0)
            .map(|row| row.index)
            .collect::<Vec<_>>();
        assert_eq!(package_group_rows, GROUP_ROWS);
        assert!(session.economy().authored_trees().is_empty());
        assert!(session.economy().spell_pickups().is_empty());

        let player_entity_index = session.state.entity_index;
        let player = &session.entities()[usize::from(player_entity_index)];
        let player_half_width = i16::from_le_bytes(
            player.record_bytes()[88..90]
                .try_into()
                .expect("player render half-width extent"),
        );
        let player_half_depth = i16::from_le_bytes(
            player.record_bytes()[90..92]
                .try_into()
                .expect("player render half-depth extent"),
        );
        let mut inside_evidence = Vec::new();
        for _ in 0..7 {
            inside_evidence.push(campaign.step(PlayerCommand::default(), 0));
            assert!(
                campaign
                    .level2_compatibility()
                    .economy()
                    .authored_trees()
                    .is_empty()
            );
        }
        assert_eq!(
            campaign.level2_compatibility().entities()[usize::from(PRODUCER_SLOT)].phase,
            56
        );
        let inside_due_evidence = campaign.step(PlayerCommand::default(), 0);
        assert!(
            campaign
                .level2_compatibility()
                .economy()
                .authored_trees()
                .is_empty()
        );
        assert!(campaign.events().iter().all(|event| !matches!(
            event,
            Level2DispatchEvent::AuthoredGroupActivated { group: 1, .. }
        )));
        assert!(crate::level1_mission::source_entities_overlap_xy(
            producer_position,
            producer_half_width,
            producer_half_depth,
            campaign.level2_compatibility().state.position,
            player_half_width,
            player_half_depth,
        ));

        let travel_command = PlayerCommand {
            flags: 1,
            ..PlayerCommand::default()
        };
        let mut travel_evidence = Vec::new();
        let mut global_before_producer = None;
        let mut producer_evidence = None;
        for _ in 0..256 {
            let global_before = campaign.level2_compatibility().source_global_rng();
            let evidence = campaign.step(travel_command, 0);
            let fired = campaign.events().iter().any(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::AuthoredGroupActivated {
                        source_slot: PRODUCER_SLOT,
                        group: 1,
                        ..
                    }
                )
            });
            travel_evidence.push(evidence.clone());
            if fired {
                global_before_producer = Some(global_before);
                producer_evidence = Some(evidence);
                break;
            }
        }
        let global_before_producer =
            global_before_producer.expect("commanded flight reaches the producer non-overlap");
        let producer_evidence = producer_evidence.expect("group 1 fires on a natural phase visit");
        assert!(!crate::level1_mission::source_entities_overlap_xy(
            producer_position,
            producer_half_width,
            producer_half_depth,
            campaign.level2_compatibility().state.position,
            player_half_width,
            player_half_depth,
        ));
        assert!(
            campaign
                .events()
                .iter()
                .all(|event| !matches!(event, Level2DispatchEvent::Error { .. }))
        );
        assert!(campaign.events().iter().any(|event| matches!(
            event,
            Level2DispatchEvent::AuthoredGroupActivated {
                pass: 0,
                source_slot: PRODUCER_SLOT,
                group: 1,
                authored_indices,
            } if authored_indices.as_ref() == GROUP_ROWS
        )));
        assert!(campaign.events().iter().any(|event| matches!(
            event,
            Level2DispatchEvent::SpellAcquisition {
                pass: 0,
                source_slot: SPELL_SLOT,
                family: 2,
                result: None,
            }
        )));

        let session = campaign.level2_compatibility();
        let spell_source = session
            .economy()
            .spell_pickups()
            .iter()
            .find(|row| row.authored_index == 31)
            .copied()
            .expect("group-1 row 31 materializes through the shared spell arena");
        assert_eq!(
            (
                spell_source.source_entity_index,
                spell_source.family,
                spell_source.action,
                spell_source.active,
            ),
            (SPELL_SLOT, 2, 7, true)
        );
        let spell_projection = session
            .shared_spells()
            .dynamic_pickups()
            .iter()
            .find(|row| row.index == SPELL_SLOT)
            .expect("row 31 has a shared class-15 callback projection");
        assert_eq!(
            (
                spell_projection.active,
                spell_projection.source_id,
                spell_projection.class,
                spell_projection.model,
                spell_projection.action,
                spell_projection.yaw,
                spell_projection.pitch,
                spell_projection.fov,
                spell_projection.particle_parameter_index,
                spell_projection.parameter_relation,
                spell_projection.special_relation,
            ),
            (
                true,
                SPELL_SLOT,
                15,
                2,
                7,
                0,
                0,
                0,
                Some(77),
                Some(59),
                EntitySpecialRelation::GlobalSpecialState,
            )
        );
        assert_eq!(
            (
                spell_projection.subspell_index,
                spell_projection.activation_value,
                spell_projection.byte_3b,
                spell_projection.mana_regen,
                spell_projection.max_mana,
                spell_projection.mana_cost,
                spell_projection.mana,
            ),
            (0, 101, 1, 0, 1_000, 1_000, 9)
        );
        assert_eq!(
            &spell_projection.record_bytes()[46..54],
            &[0, 0, 0, 0, 0, 0, 101, 0]
        );
        assert_eq!(
            &spell_projection.record_bytes()[140..152],
            &[0, 0, 0, 0, 232, 3, 0, 0, 9, 0, 0, 0]
        );
        assert_eq!(
            &spell_projection.record_bytes()[30..40],
            &[178, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(&spell_projection.record_bytes()[44..46], &[0, 0]);
        assert_eq!(
            &spell_projection.record_bytes()[86..96],
            &[75, 0, 0, 3, 0, 3, 0, 5, 77, 0]
        );
        assert_eq!(
            &spell_projection.record_bytes()[164..168],
            &60_u32.to_le_bytes()
        );
        assert_eq!(
            &spell_projection.record_bytes()[168..176],
            &[2, 0, 0, 0, 1, 0, 0, 0]
        );

        let trees = session.economy().authored_trees().to_vec();
        assert_eq!(
            trees
                .iter()
                .map(|row| row.authored_index)
                .collect::<Vec<_>>(),
            TREE_ROWS
        );
        assert!(trees.iter().all(|tree| {
            session.mission().authored_features().iter().any(|feature| {
                feature.index == tree.authored_index
                    && feature.activation_group == 1
                    && (feature.kind, feature.subtype) == (2, 0)
            })
        }));
        let first_tree = trees
            .iter()
            .find(|row| row.authored_index == TREE_ROWS[0])
            .expect("first source-ordered Level 1 tree");
        assert_eq!(first_tree.source_entity_index, FIRST_TREE_SLOT);
        let next_rng16 = |seed: u16| seed.wrapping_mul(9_377).wrapping_add(9_439);
        let pass_global = global_before_producer
            .wrapping_mul(9_377)
            .wrapping_add(9_439);
        let life_rng = next_rng16(
            first_tree
                .source_entity_index
                .wrapping_add(pass_global as u16),
        );
        let x_rng = next_rng16(life_rng);
        let y_rng = next_rng16(x_rng);
        let particle_rng = next_rng16(y_rng);
        let feature = session
            .mission()
            .authored_features()
            .iter()
            .find(|row| row.index == TREE_ROWS[0])
            .expect("row 155 remains in the package census");
        let authored_position = feature.world_position(
            session
                .canonical_terrain()
                .ground_height(feature.world_position(0)),
        );
        let expected_x = authored_position
            .x
            .wrapping_add((x_rng & 0x3f).wrapping_sub(32));
        let expected_y = authored_position
            .y
            .wrapping_add((y_rng & 0x3f).wrapping_sub(32));
        assert_eq!(
            (
                first_tree.source_class,
                first_tree.model,
                first_tree.action,
                first_tree.entity_rng,
                first_tree.particle_index,
                first_tree.control_flags,
                first_tree.position.x,
                first_tree.position.y,
                first_tree.position.z,
            ),
            (
                2,
                0,
                0,
                particle_rng,
                if particle_rng & 1 == 0 { 83 } else { 84 },
                0x0002_000c,
                expected_x,
                expected_y,
                session.canonical_terrain().ground_height(crate::Position {
                    x: expected_x,
                    y: expected_y,
                    z: authored_position.z,
                }),
            )
        );
        assert!(
            session
                .economy()
                .map_chains()
                .entity_chain((expected_x >> 8) as u8, (expected_y >> 8) as u8)
                .contains(&FIRST_TREE_SLOT)
        );

        let receipts = session.cf10_production_schedule_receipts();
        for source_slot in
            std::iter::once(SPELL_SLOT).chain(trees.iter().map(|row| row.source_entity_index))
        {
            assert!(receipts.iter().any(|receipt| matches!(
                receipt,
                ScheduleReceipt::SlotResolved {
                    pass: SourcePass::UpdateEntities,
                    scan: 0,
                    slot,
                    kind: VisitKind::Callback,
                    ..
                } if *slot == source_slot
            )));
            assert!(receipts.iter().any(|receipt| matches!(
                receipt,
                ScheduleReceipt::CallbackCompleted {
                    pass: SourcePass::UpdateEntities,
                    scan: 0,
                    slot,
                    ..
                } if *slot == source_slot
            )));
            assert!(receipts.iter().any(|receipt| matches!(
                receipt,
                ScheduleReceipt::EpilogueCompleted {
                    pass: SourcePass::UpdateEntities,
                    scan: 0,
                    slot,
                    ..
                } if *slot == source_slot
            )));
        }

        let producer_events = campaign.events().to_vec();
        let pickup_position = spell_source.position;
        campaign
            .level2_compatibility_mut()
            .set_level2_entity_position(player_entity_index, pickup_position)
            .expect("move the real local player through the production position carrier");
        let mut acquisition_evidence = Vec::new();
        let mut acquired = None;
        for attempt in 0..4 {
            acquisition_evidence.push(campaign.step(PlayerCommand::default(), 0));
            assert!(
                campaign
                    .events()
                    .iter()
                    .all(|event| !matches!(event, Level2DispatchEvent::Error { .. }))
            );
            if attempt == 0 {
                let reclaimed_producer =
                    &campaign.level2_compatibility().entities()[usize::from(PRODUCER_SLOT)];
                assert_eq!(
                    (reclaimed_producer.active, reclaimed_producer.class),
                    (false, 0),
                    "the hidden producer is reclaimed on the following source pass"
                );
                assert!(campaign.events().iter().all(|event| !matches!(
                    event,
                    Level2DispatchEvent::AuthoredGroupActivated { group: 1, .. }
                )));
            }
            acquired = campaign.events().iter().find_map(|event| match event {
                Level2DispatchEvent::SpellAcquisition {
                    source_slot: SPELL_SLOT,
                    family: 2,
                    result: Some(outcome),
                    ..
                } => Some(*outcome),
                _ => None,
            });
            if acquired.is_some() {
                break;
            }
        }
        let acquired = acquired.expect("the naturally phased family-2 source is acquired");
        assert_eq!(
            (
                acquired.family,
                acquired.source_slot,
                acquired.holder_action,
                acquired.parent_entity_index,
                acquired.presentation_timer,
            ),
            (2, SPELL_SLOT, 6, player_entity_index, 64)
        );
        let session = campaign.level2_compatibility();
        let acquired_projection = session
            .shared_spells()
            .dynamic_pickups()
            .iter()
            .find(|row| row.index == SPELL_SLOT)
            .expect("acquired family-2 projection remains the source holder");
        assert_eq!(acquired_projection.action, 6);
        assert_eq!(
            u16::from_le_bytes(
                acquired_projection.record_bytes()[44..46]
                    .try_into()
                    .expect("source parent relation extent")
            ),
            player_entity_index
        );
        let local_player = &session.shared_spells().players()[0];
        assert_eq!(local_player.enabled_spell_holders[2], SPELL_SLOT);
        assert_eq!(local_player.acquired_spell_flags[2], 1);
        assert_eq!(session.players()[0].enabled_spell_holders[2], SPELL_SLOT);
        assert_eq!(session.players()[0].acquired_spell_flags[2], 1);
        let pickup_sounds = session
            .shared_spells()
            .activation_events()
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    FlightEvent::PrepareSound {
                        entity_index,
                        player_index: -1,
                        sound_id: 18,
                        ..
                    } if *entity_index == player_entity_index
                )
            })
            .count();
        assert_eq!(pickup_sounds, 1);

        let first_final_hash = campaign.state_hash();
        let first_events = campaign.events().to_vec();
        let first_trees = campaign
            .level2_compatibility()
            .economy()
            .authored_trees()
            .to_vec();
        let first_spell_projection = campaign
            .level2_compatibility()
            .shared_spells()
            .dynamic_pickups()
            .to_vec();
        campaign.reset();
        assert_eq!(campaign.state_hash(), initial_hash);
        assert!(
            campaign
                .level2_compatibility()
                .economy()
                .authored_trees()
                .is_empty()
        );
        assert!(
            campaign
                .level2_compatibility()
                .shared_spells()
                .dynamic_pickups()
                .is_empty()
        );

        let mut replay_inside = Vec::new();
        for _ in 0..7 {
            replay_inside.push(campaign.step(PlayerCommand::default(), 0));
        }
        let replay_inside_due_evidence = campaign.step(PlayerCommand::default(), 0);
        assert_eq!(replay_inside, inside_evidence);
        assert_eq!(replay_inside_due_evidence, inside_due_evidence);
        let mut replay_travel_evidence = Vec::new();
        for _ in 0..travel_evidence.len() {
            replay_travel_evidence.push(campaign.step(travel_command, 0));
        }
        assert_eq!(replay_travel_evidence, travel_evidence);
        assert_eq!(
            replay_travel_evidence.last(),
            Some(&producer_evidence),
            "replay reaches the same source producer visit"
        );
        assert_eq!(campaign.events(), producer_events);
        campaign
            .level2_compatibility_mut()
            .set_level2_entity_position(player_entity_index, pickup_position)
            .expect("replay uses the same production position carrier");
        let mut replay_acquisition_evidence = Vec::new();
        for _ in 0..acquisition_evidence.len() {
            replay_acquisition_evidence.push(campaign.step(PlayerCommand::default(), 0));
        }
        assert_eq!(replay_acquisition_evidence, acquisition_evidence);
        assert_eq!(campaign.events(), first_events);
        assert_eq!(campaign.state_hash(), first_final_hash);
        assert_eq!(
            campaign.level2_compatibility().economy().authored_trees(),
            first_trees
        );
        assert_eq!(
            campaign
                .level2_compatibility()
                .shared_spells()
                .dynamic_pickups(),
            first_spell_projection
        );
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
            Vec::<(u8, u8)>::new(),
            "every census-proven live root must have a compiled capability"
        );

        // The production route consumes the same descriptor and registry,
        // derives the local wizard from runtime -> player -> entity relations,
        // and creates one resettable Rust authority without a Level-3 branch.
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL3)
            .expect("public Level 3 descriptor opens through the shared materializer");
        assert_eq!(
            campaign
                .inspection()
                .entities()
                .iter()
                .filter(|row| row.active)
                .count(),
            252
        );
        assert_eq!(campaign.level2_compatibility().state.entity_index, 252);
        let controller = &campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities[94];
        assert_eq!(
            (
                controller.class,
                controller.model,
                controller.action,
                controller.life,
                controller.subspell_index,
                i32::from_le_bytes(controller.record_bytes()[20..24].try_into().unwrap()),
                controller.phase,
            ),
            (14, 1, 6, 3, 48, 14, 95),
            "the real Level 3 terrain-strip row must be rehydrated losslessly"
        );
        let reset_state_hash = campaign.state_hash();
        let reset_terrain_hash = campaign.terrain_hash();
        let reset_rows = campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .filter(|row| row.active)
            .map(|row| (row.index, (row.class, row.action)))
            .collect::<BTreeMap<_, _>>();
        let imported_family_before = campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .filter(|row| {
                row.active
                    && matches!(
                        (row.class, row.model, row.action),
                        (2, 0, 0) | (2, 1, 3) | (5, 9, 72)
                    )
            })
            .map(|row| {
                (
                    row.index,
                    row.class,
                    row.model,
                    row.phase,
                    i32::from_le_bytes(row.record_bytes()[20..24].try_into().unwrap()),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            imported_family_before
                .iter()
                .filter(|(_, class, model, _, _)| (*class, *model) == (2, 0))
                .count(),
            37,
            "the production proof must cover every imported Level 3 tree"
        );
        assert_eq!(
            imported_family_before
                .iter()
                .filter(|(_, class, model, _, _)| (*class, *model) == (2, 1))
                .count(),
            12,
            "the production proof must cover every imported Level 3 stone"
        );
        assert_eq!(
            imported_family_before
                .iter()
                .filter(|(_, class, model, _, _)| (*class, *model) == (5, 9))
                .count(),
            11,
            "the production proof must cover every imported Level 3 model-9 row"
        );
        let _ = campaign.step(PlayerCommand::default(), 0);
        let receipts = campaign
            .level2_compatibility()
            .cf10_production_schedule_receipts();
        let resolved = receipts
            .iter()
            .filter_map(|receipt| match receipt {
                ScheduleReceipt::SlotResolved {
                    pass: SourcePass::UpdateEntities,
                    scan: 0,
                    slot,
                    selector,
                    kind,
                } => Some((*slot, selector, *kind)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let unexpected_resolved = resolved
            .iter()
            .filter(|(slot, _, _)| !reset_rows.contains_key(slot))
            .map(|(slot, selector, kind)| (*slot, selector.registry_key(), *kind))
            .collect::<Vec<_>>();
        assert_eq!(
            resolved
                .iter()
                .map(|(slot, _, _)| *slot)
                .collect::<Vec<_>>(),
            reset_rows.keys().copied().collect::<Vec<_>>(),
            "the first descriptor pass must visit exactly the 252 reset rows in numeric source order; unexpected visits: {unexpected_resolved:?}"
        );
        assert!(
            resolved
                .iter()
                .all(|(_, _, kind)| *kind == VisitKind::Callback),
            "all fifteen census selector families are real callbacks"
        );
        for (slot, selector, _) in &resolved {
            let (expected_class, expected_action) = reset_rows[slot];
            assert_eq!(
                selector.registry_key().selector,
                BehaviorSelector::ClassAction {
                    class: expected_class,
                    action: expected_action,
                },
                "slot {slot} must be resolved from its lossless reset selector"
            );
            assert!(receipts.iter().any(|receipt| matches!(
                receipt,
                ScheduleReceipt::CallbackCompleted {
                    pass: SourcePass::UpdateEntities,
                    scan: 0,
                    slot: completed_slot,
                    selector: completed,
                } if completed_slot == slot && completed == *selector
            )));
            assert!(receipts.iter().any(|receipt| matches!(
                receipt,
                ScheduleReceipt::EpilogueCompleted {
                    pass: SourcePass::UpdateEntities,
                    scan: 0,
                    slot: completed_slot,
                    original,
                    ..
                } if completed_slot == slot && original == *selector
            )));
        }
        assert!(receipts.iter().all(|receipt| !matches!(
            receipt,
            ScheduleReceipt::CleanupCompleted {
                pass: SourcePass::UpdateEntities,
                scan: 0,
                ..
            }
        )));
        assert!(receipts.iter().any(|receipt| matches!(
            receipt,
            ScheduleReceipt::ScanFinished {
                pass: SourcePass::UpdateEntities,
                scan: 0,
                eligible_visits: 252,
            }
        )));
        assert_eq!(
            receipts.len(),
            763,
            "five phases + scan bounds + 252 resolve/callback/epilogue triples"
        );
        let controller = &campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities[94];
        assert_eq!(
            (
                controller.life,
                controller.subspell_index,
                i32::from_le_bytes(controller.record_bytes()[20..24].try_into().unwrap()),
                controller.phase,
            ),
            (3, 48, 14, 96),
            "the inert controller callback still receives exactly one UpdateEntities epilogue"
        );
        for (slot, class, model, phase_before, countdown_before) in imported_family_before {
            let row = &campaign
                .level2_compatibility()
                .session
                .state
                .live
                .economy
                .entities[usize::from(slot)];
            assert_eq!(
                row.phase,
                phase_before.wrapping_add(1),
                "imported {class}/{model} slot {slot} must receive one scheduler epilogue"
            );
            match (class, model) {
                (2, 0 | 1) => {
                    assert_ne!(
                        row.control_flags & 0x0002_0000,
                        0,
                        "class-2 callback must set the source byte-2 lifecycle bit at slot {slot}"
                    );
                }
                (5, 9) => assert_eq!(
                    i32::from_le_bytes(row.record_bytes()[20..24].try_into().unwrap()),
                    countdown_before.wrapping_sub(1),
                    "model-9 action72 must pre-decrement its countdown at slot {slot}"
                ),
                _ => unreachable!(),
            }
        }
        assert!(campaign.events().iter().all(|event| !matches!(
            event,
            crate::native_world::Level2DispatchEvent::Error { .. }
        )));
        campaign.reset();
        assert_eq!(campaign.state_hash(), reset_state_hash);
        assert_eq!(campaign.terrain_hash(), reset_terrain_hash);

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
    fn cf14_level3_model9_countdown_uses_package_pose_and_stage_handoff() {
        let mut campaign = CampaignSession::open(
            level_three_descriptor_fixture_path(),
            CampaignSelection::LEVEL3,
        )
        .expect("qualified Level 3 descriptor opens");
        let before = campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities[93]
            .clone();
        let countdown = i32::from_le_bytes(before.record_bytes()[20..24].try_into().unwrap());
        assert_eq!(
            countdown, 15,
            "the qualified descriptor preserves the captured source pre-decrement clock"
        );
        let stage_before = campaign
            .level2_compatibility()
            .session
            .state
            .live
            .core
            .runtime
            .stage_variables[2];
        assert_eq!(
            (stage_before.kind, stage_before.flags, stage_before.word_2),
            (1, 1, 0)
        );

        for _ in 0..=countdown {
            let _ = campaign.step(PlayerCommand::default(), 0);
        }

        let after = &campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities[93];
        assert_eq!(
            after.action, 79,
            "sub_12330 must replace intermediate action 73"
        );
        assert_eq!(
            i32::from_le_bytes(after.record_bytes()[20..24].try_into().unwrap()),
            400
        );
        assert_eq!(after.actual_speed, 20);
        assert_eq!(after.particle_parameter_index, Some(201));
        assert_eq!((after.record_bytes()[96], after.record_bytes()[97]), (0, 1));
        assert_eq!(
            (
                i16::from_le_bytes(after.record_bytes()[86..88].try_into().unwrap()),
                i16::from_le_bytes(after.record_bytes()[88..90].try_into().unwrap()),
                i16::from_le_bytes(after.record_bytes()[90..92].try_into().unwrap()),
                i16::from_le_bytes(after.record_bytes()[92..94].try_into().unwrap()),
            ),
            (128, 74, 74, 128),
            "SetEntityIndexAndRot must use package particle 201 plus remc2's render-mode table"
        );
        assert_eq!(&after.record_bytes()[76..80], &[2, 1, 0, 0]);
        assert_eq!(after.position, before.position);
        assert_eq!(after.entity_rng, before.entity_rng);
        assert_eq!(after.phase, before.phase.wrapping_add(countdown as u8 + 1));
        assert_eq!(
            campaign
                .level2_compatibility()
                .session
                .state
                .live
                .core
                .runtime
                .stage_variables[2]
                .word_2,
            0x0b00,
            "all eleven source-ordered model-9 rows advance the shared StageVar once"
        );
        assert!(campaign.events().iter().all(|event| !matches!(
            event,
            crate::native_world::Level2DispatchEvent::Error { .. }
        )));

        let before_action79 = campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .filter(|row| row.active && row.class == 5 && row.model == 9)
            .map(|row| (row.index, row.phase, row.entity_rng, row.position))
            .collect::<Vec<_>>();
        assert_eq!(before_action79.len(), 11);
        assert_eq!(
            before_action79
                .iter()
                .filter(|(_, phase, _, _)| phase & 7 == 0)
                .map(|(slot, _, _, _)| *slot)
                .collect::<Vec<_>>(),
            vec![248],
            "the captured phases make slot 248 the only first-pass steering row"
        );
        let _ = campaign.step(PlayerCommand::default(), 0);
        let action79_rows = &campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities;
        for (slot, phase, entity_rng, _) in &before_action79 {
            let row = &action79_rows[usize::from(*slot)];
            assert_eq!(row.action, 79);
            assert_eq!(row.phase, phase.wrapping_add(1));
            assert_eq!(row.entity_rng, *entity_rng);
        }
        let packed_target = match campaign
            .level2_compatibility()
            .session
            .state
            .live
            .core
            .runtime
            .stage_variables[2]
            .relation
        {
            crate::native_world::RuntimeRelation::AxisScalar(value) => value,
            relation => panic!("model-9 StageVar target is not an axis: {relation:?}"),
        };
        let target = crate::Position {
            x: packed_target as u16,
            y: (packed_target >> 16) as u16,
            z: 0,
        };
        assert_eq!(
            i16::from_le_bytes(
                action79_rows[248].record_bytes()[36..38]
                    .try_into()
                    .unwrap()
            ),
            crate::heading_for_positions(action79_rows[248].position, target) as i16
        );
        assert!(campaign.events().iter().all(|event| !matches!(
            event,
            crate::native_world::Level2DispatchEvent::Error { .. }
        )));
    }

    #[test]
    fn cf14_level3_multipart_shares_stage_target_on_eight_phase_cadence() {
        let mut campaign = CampaignSession::open(
            level_three_descriptor_fixture_path(),
            CampaignSelection::LEVEL3,
        )
        .expect("qualified Level 3 descriptor opens");
        let entities = &campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities;
        assert_eq!(entities[201].phase, 5);
        assert_eq!(&entities[201].record_bytes()[78..80], &[0, 0]);
        assert_eq!(
            u16::from_le_bytes(entities[133].record_bytes()[78..80].try_into().unwrap()),
            93,
            "the first source-ordered root carries the shared target"
        );

        for _ in 0..3 {
            let _ = campaign.step(PlayerCommand::default(), 0);
        }
        let before = campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities[201]
            .clone();
        let target_position = campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities[93]
            .position;
        assert_eq!(before.phase, 8);
        assert_eq!(&before.record_bytes()[78..80], &[0, 0]);

        let _ = campaign.step(PlayerCommand::default(), 0);
        let after = &campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities[201];
        assert_eq!(after.phase, 9);
        assert_eq!(
            u16::from_le_bytes(after.record_bytes()[78..80].try_into().unwrap()),
            93
        );
        assert_ne!(after.position, before.position);
        assert_eq!(after.entity_rng, before.entity_rng);
        assert_eq!(
            i16::from_le_bytes(after.record_bytes()[36..38].try_into().unwrap()),
            crate::heading_for_positions(after.position, target_position) as i16
        );
        assert!(campaign.events().iter().all(|event| !matches!(
            event,
            crate::native_world::Level2DispatchEvent::Error { .. }
        )));
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
