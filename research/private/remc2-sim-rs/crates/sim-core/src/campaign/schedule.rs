//! Literal source-order scheduling primitives for campaign behavior.
//!
//! The two source sites deliberately remain separate algorithms:
//! [`run_update_entities`] performs exactly one ascending live-slot scan, while
//! [`run_apply_events`] repeats full ascending scans while a callback-capable
//! source branch requests `runagain`; terminal-only rows complete in the
//! current scan without forcing a redundant retry. Callers resolve every slot
//! at visit time. Consequently a higher slot allocated during a callback is
//! visible in the current scan, whereas an earlier or same slot is deferred
//! until a later scan or invocation.

use std::{error::Error, fmt, num::NonZeroU16};

/// First source entity slot visited by the remc2 schedulers.
pub const FIRST_SOURCE_SLOT: u16 = 1;
/// Last source entity slot visited by the remc2 schedulers.
pub const LAST_SOURCE_SLOT: u16 = 999;

/// The two source scheduling sites.  They are not interchangeable modes of one
/// loop: their scan and retry laws differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourcePass {
    ApplyEvents,
    UpdateEntities,
}

/// Preparation and following-pass hooks whose order is scheduler-owned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassPhase {
    RngPrepass,
    FollowingPassCleanup,
    RebuildMemberships,
    PreDispatch,
    PostDispatch,
}

/// What the source site does with a currently eligible row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisitKind {
    Callback,
    NoCallback,
    Terminal,
}

/// A source-site-specific plan for one live row.
///
/// `epilogue` and `cleanup` are opaque instructions owned by the adapter.  The
/// scheduler preserves their placement without embedding game-specific state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisitPlan<Selector, Epilogue, Cleanup> {
    pub selector: Selector,
    pub kind: VisitKind,
    pub epilogue: Option<Epilogue>,
    pub cleanup: Option<Cleanup>,
}

impl<Selector, Epilogue, Cleanup> VisitPlan<Selector, Epilogue, Cleanup> {
    pub fn callback(selector: Selector) -> Self {
        Self {
            selector,
            kind: VisitKind::Callback,
            epilogue: None,
            cleanup: None,
        }
    }

    pub fn no_callback(selector: Selector) -> Self {
        Self {
            selector,
            kind: VisitKind::NoCallback,
            epilogue: None,
            cleanup: None,
        }
    }

    pub fn terminal(selector: Selector) -> Self {
        Self {
            selector,
            kind: VisitKind::Terminal,
            epilogue: None,
            cleanup: None,
        }
    }
}

/// Live resolution supplied by the source adapter at the instant a slot is
/// visited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotResolution<Selector, Epilogue, Cleanup> {
    Inactive,
    Eligible(VisitPlan<Selector, Epilogue, Cleanup>),
    /// A live row exists, but its selector has no admitted dispatch meaning.
    Unknown {
        selector: Selector,
    },
}

/// The occupant observed after a callback and before the original visit's
/// epilogue.  This explicit re-resolution preserves self-slot replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurrentOccupant<Selector> {
    Inactive,
    Known { selector: Selector, kind: VisitKind },
}

/// Ordered, state-bearing scheduler evidence.  Receipts are emitted at the
/// real scheduling seams; consumers need not infer order from decorative log
/// text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleReceipt<Selector> {
    Phase {
        pass: SourcePass,
        phase: PassPhase,
    },
    ScanStarted {
        pass: SourcePass,
        scan: u16,
    },
    SlotResolved {
        pass: SourcePass,
        scan: u16,
        slot: u16,
        selector: Selector,
        kind: VisitKind,
    },
    CallbackCompleted {
        pass: SourcePass,
        scan: u16,
        slot: u16,
        selector: Selector,
    },
    EpilogueCompleted {
        pass: SourcePass,
        scan: u16,
        slot: u16,
        original: Selector,
        current: CurrentOccupant<Selector>,
    },
    CleanupCompleted {
        pass: SourcePass,
        scan: u16,
        slot: u16,
        current: CurrentOccupant<Selector>,
    },
    ScanFinished {
        pass: SourcePass,
        scan: u16,
        eligible_visits: u16,
    },
}

/// Aggregate result for one source scheduler invocation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScheduleStats {
    pub scans: u16,
    pub eligible_visits: u32,
    pub callbacks: u32,
    pub epilogues: u32,
    pub cleanups: u32,
}

/// The literal control steps of one ordinary UpdateEntities invocation.
///
/// Production and the generic adapter runner consume the same iterator, so
/// phase order and numeric cursor ownership cannot drift into two friendly
/// loops that merely happen to look alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateScheduleStep {
    Phase(PassPhase),
    ScanStarted,
    VisitSlot(u16),
    ScanFinished,
}

#[derive(Debug, Clone)]
pub struct UpdateEntitiesSchedule {
    state: u16,
}

impl UpdateEntitiesSchedule {
    #[must_use]
    pub const fn new() -> Self {
        Self { state: 0 }
    }
}

impl Default for UpdateEntitiesSchedule {
    fn default() -> Self {
        Self::new()
    }
}

impl Iterator for UpdateEntitiesSchedule {
    type Item = UpdateScheduleStep;

    fn next(&mut self) -> Option<Self::Item> {
        let step = match self.state {
            0 => UpdateScheduleStep::Phase(PassPhase::RngPrepass),
            1 => UpdateScheduleStep::Phase(PassPhase::FollowingPassCleanup),
            2 => UpdateScheduleStep::Phase(PassPhase::RebuildMemberships),
            3 => UpdateScheduleStep::Phase(PassPhase::PreDispatch),
            4 => UpdateScheduleStep::ScanStarted,
            5..=1003 => UpdateScheduleStep::VisitSlot(self.state - 4),
            1004 => UpdateScheduleStep::ScanFinished,
            1005 => UpdateScheduleStep::Phase(PassPhase::PostDispatch),
            _ => return None,
        };
        self.state = self.state.saturating_add(1);
        Some(step)
    }
}

/// Typed scheduler failure.  Adapter failures are never converted into an
/// inactive row or a successful partial pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleError<Selector, AdapterError> {
    UnknownDispatch {
        pass: SourcePass,
        scan: u16,
        slot: u16,
        selector: Selector,
    },
    Adapter {
        pass: SourcePass,
        scan: u16,
        slot: Option<u16>,
        source: AdapterError,
    },
    ApplyNoProgress {
        scan: u16,
        eligible_visits: u16,
    },
    ApplyScanLimit {
        max_scans: u16,
    },
}

impl<Selector: fmt::Debug, AdapterError: fmt::Display> fmt::Display
    for ScheduleError<Selector, AdapterError>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownDispatch {
                pass,
                scan,
                slot,
                selector,
            } => write!(
                formatter,
                "unknown {pass:?} selector {selector:?} at scan {scan}, slot {slot}"
            ),
            Self::Adapter {
                pass,
                scan,
                slot,
                source,
            } => write!(
                formatter,
                "{pass:?} adapter failed at scan {scan}, slot {slot:?}: {source}"
            ),
            Self::ApplyNoProgress {
                scan,
                eligible_visits,
            } => write!(
                formatter,
                "ApplyEvents scan {scan} found {eligible_visits} eligible rows without progress"
            ),
            Self::ApplyScanLimit { max_scans } => write!(
                formatter,
                "ApplyEvents did not quiesce within {max_scans} full scans"
            ),
        }
    }
}

impl<Selector: fmt::Debug, AdapterError: Error + 'static> Error
    for ScheduleError<Selector, AdapterError>
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Adapter { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Adapter boundary used by both literal schedulers.
///
/// `Progress` must include every state coordinate that can change whether an
/// ApplyEvents row remains eligible.  It is used only to turn a non-quiescing
/// implementation into a typed error; retry itself is scheduler-owned and is
/// based solely on observing an eligible row.
pub trait SourceSchedulerWorld {
    type Selector: Clone + fmt::Debug + Eq;
    type Epilogue: Clone;
    type Cleanup: Clone;
    type Progress: Eq;
    type Error;

    fn phase(&mut self, pass: SourcePass, phase: PassPhase) -> Result<(), Self::Error>;

    fn resolve(
        &self,
        pass: SourcePass,
        scan: u16,
        slot: u16,
    ) -> SlotResolution<Self::Selector, Self::Epilogue, Self::Cleanup>;

    fn callback(
        &mut self,
        pass: SourcePass,
        scan: u16,
        slot: u16,
        selector: &Self::Selector,
    ) -> Result<(), Self::Error>;

    fn epilogue(
        &mut self,
        pass: SourcePass,
        scan: u16,
        slot: u16,
        instruction: &Self::Epilogue,
        current: &CurrentOccupant<Self::Selector>,
    ) -> Result<(), Self::Error>;

    fn cleanup(
        &mut self,
        pass: SourcePass,
        scan: u16,
        slot: u16,
        instruction: &Self::Cleanup,
        current: &CurrentOccupant<Self::Selector>,
    ) -> Result<(), Self::Error>;

    fn progress(&self, pass: SourcePass) -> Self::Progress;

    fn record(&mut self, receipt: ScheduleReceipt<Self::Selector>);
}

/// Perform the ordinary remc2 entity update: one ascending scan with live
/// resolution at each numeric slot.
pub fn run_update_entities<World>(
    world: &mut World,
) -> Result<ScheduleStats, ScheduleError<World::Selector, World::Error>>
where
    World: SourceSchedulerWorld,
{
    const PASS: SourcePass = SourcePass::UpdateEntities;
    let mut stats = ScheduleStats {
        scans: 1,
        ..ScheduleStats::default()
    };

    let mut eligible_visits = 0_u16;
    for step in UpdateEntitiesSchedule::new() {
        match step {
            UpdateScheduleStep::Phase(phase) => run_phase(world, PASS, phase)?,
            UpdateScheduleStep::ScanStarted => {
                world.record(ScheduleReceipt::ScanStarted {
                    pass: PASS,
                    scan: 0,
                });
            }
            UpdateScheduleStep::VisitSlot(slot) => match world.resolve(PASS, 0, slot) {
                SlotResolution::Inactive => {}
                SlotResolution::Unknown { selector } => {
                    return Err(ScheduleError::UnknownDispatch {
                        pass: PASS,
                        scan: 0,
                        slot,
                        selector,
                    });
                }
                SlotResolution::Eligible(plan) => {
                    eligible_visits = eligible_visits.saturating_add(1);
                    visit(world, PASS, 0, slot, plan, &mut stats)?;
                }
            },
            UpdateScheduleStep::ScanFinished => {
                world.record(ScheduleReceipt::ScanFinished {
                    pass: PASS,
                    scan: 0,
                    eligible_visits,
                });
            }
        }
    }
    stats.eligible_visits = u32::from(eligible_visits);
    Ok(stats)
}

/// Perform the remc2 ApplyEvents fixpoint scan.
///
/// A callback's return value cannot suppress another scan. Callback and
/// source-table no-callback branches request a restart from slot 1; terminal
/// hide/reclaim branches do not set remc2's `runagain` flag. The progress
/// marker and bound only reject a broken/non-quiescing adapter.
///
/// The current Level-2 package contract contains the arena after remc2's
/// one-time `GenerateEvents` batches. Therefore this load-time primitive has
/// no ordinary-frame caller; inserting it into `step_outer_turn` would add an
/// extra RNG draw and repeat construction work that the package already
/// materialized.
pub fn run_apply_events<World>(
    world: &mut World,
    max_scans: NonZeroU16,
) -> Result<ScheduleStats, ScheduleError<World::Selector, World::Error>>
where
    World: SourceSchedulerWorld,
{
    const PASS: SourcePass = SourcePass::ApplyEvents;
    let mut stats = ScheduleStats::default();
    run_phase(world, PASS, PassPhase::RngPrepass)?;

    for scan in 0..max_scans.get() {
        let before = world.progress(PASS);
        let (eligible, rescan_requests) = run_scan(world, PASS, scan, &mut stats)?;
        stats.scans = stats.scans.saturating_add(1);
        stats.eligible_visits = stats.eligible_visits.saturating_add(u32::from(eligible));
        if rescan_requests == 0 {
            return Ok(stats);
        }
        if world.progress(PASS) == before {
            return Err(ScheduleError::ApplyNoProgress {
                scan,
                eligible_visits: eligible,
            });
        }
    }

    Err(ScheduleError::ApplyScanLimit {
        max_scans: max_scans.get(),
    })
}

fn run_phase<World>(
    world: &mut World,
    pass: SourcePass,
    phase: PassPhase,
) -> Result<(), ScheduleError<World::Selector, World::Error>>
where
    World: SourceSchedulerWorld,
{
    world
        .phase(pass, phase)
        .map_err(|source| ScheduleError::Adapter {
            pass,
            scan: 0,
            slot: None,
            source,
        })?;
    world.record(ScheduleReceipt::Phase { pass, phase });
    Ok(())
}

fn run_scan<World>(
    world: &mut World,
    pass: SourcePass,
    scan: u16,
    stats: &mut ScheduleStats,
) -> Result<(u16, u16), ScheduleError<World::Selector, World::Error>>
where
    World: SourceSchedulerWorld,
{
    world.record(ScheduleReceipt::ScanStarted { pass, scan });
    let mut eligible_visits = 0_u16;
    let mut rescan_requests = 0_u16;

    for slot in FIRST_SOURCE_SLOT..=LAST_SOURCE_SLOT {
        match world.resolve(pass, scan, slot) {
            SlotResolution::Inactive => {}
            SlotResolution::Unknown { selector } => {
                return Err(ScheduleError::UnknownDispatch {
                    pass,
                    scan,
                    slot,
                    selector,
                });
            }
            SlotResolution::Eligible(plan) => {
                eligible_visits = eligible_visits.saturating_add(1);
                if matches!(plan.kind, VisitKind::Callback | VisitKind::NoCallback) {
                    rescan_requests = rescan_requests.saturating_add(1);
                }
                visit(world, pass, scan, slot, plan, stats)?;
            }
        }
    }

    world.record(ScheduleReceipt::ScanFinished {
        pass,
        scan,
        eligible_visits,
    });
    Ok((eligible_visits, rescan_requests))
}

fn visit<World>(
    world: &mut World,
    pass: SourcePass,
    scan: u16,
    slot: u16,
    plan: VisitPlan<World::Selector, World::Epilogue, World::Cleanup>,
    stats: &mut ScheduleStats,
) -> Result<(), ScheduleError<World::Selector, World::Error>>
where
    World: SourceSchedulerWorld,
{
    world.record(ScheduleReceipt::SlotResolved {
        pass,
        scan,
        slot,
        selector: plan.selector.clone(),
        kind: plan.kind,
    });

    if plan.kind == VisitKind::Callback {
        world
            .callback(pass, scan, slot, &plan.selector)
            .map_err(|source| ScheduleError::Adapter {
                pass,
                scan,
                slot: Some(slot),
                source,
            })?;
        stats.callbacks = stats.callbacks.saturating_add(1);
        world.record(ScheduleReceipt::CallbackCompleted {
            pass,
            scan,
            slot,
            selector: plan.selector.clone(),
        });
    }

    let current = current_occupant(world, pass, scan, slot)?;
    if let Some(instruction) = &plan.epilogue {
        world
            .epilogue(pass, scan, slot, instruction, &current)
            .map_err(|source| ScheduleError::Adapter {
                pass,
                scan,
                slot: Some(slot),
                source,
            })?;
        stats.epilogues = stats.epilogues.saturating_add(1);
        world.record(ScheduleReceipt::EpilogueCompleted {
            pass,
            scan,
            slot,
            original: plan.selector.clone(),
            current: current.clone(),
        });
    }

    if let Some(instruction) = &plan.cleanup {
        // Epilogues may replace or deactivate the occupant, so cleanup gets a
        // fresh identity rather than the pre-epilogue snapshot.
        let cleanup_current = current_occupant(world, pass, scan, slot)?;
        world
            .cleanup(pass, scan, slot, instruction, &cleanup_current)
            .map_err(|source| ScheduleError::Adapter {
                pass,
                scan,
                slot: Some(slot),
                source,
            })?;
        stats.cleanups = stats.cleanups.saturating_add(1);
        world.record(ScheduleReceipt::CleanupCompleted {
            pass,
            scan,
            slot,
            current: cleanup_current,
        });
    }
    Ok(())
}

fn current_occupant<World>(
    world: &World,
    pass: SourcePass,
    scan: u16,
    slot: u16,
) -> Result<CurrentOccupant<World::Selector>, ScheduleError<World::Selector, World::Error>>
where
    World: SourceSchedulerWorld,
{
    match world.resolve(pass, scan, slot) {
        SlotResolution::Inactive => Ok(CurrentOccupant::Inactive),
        SlotResolution::Eligible(plan) => Ok(CurrentOccupant::Known {
            selector: plan.selector,
            kind: plan.kind,
        }),
        SlotResolution::Unknown { selector } => Err(ScheduleError::UnknownDispatch {
            pass,
            scan,
            slot,
            selector,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        PlayerCommand,
        campaign::{dispatch::CAMPAIGN_BEHAVIOR_REGISTRY, spec::BehaviorSelector},
        native_world::{Level2DispatchEvent, NativeWorld},
    };
    use std::collections::BTreeMap;
    use std::path::PathBuf;

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

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Epilogue {
        Keep,
        Deactivate,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Cleanup {
        Reclaim,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Row {
        selector: &'static str,
        kind: VisitKind,
        active: bool,
        epilogue: Option<Epilogue>,
        cleanup: Option<Cleanup>,
    }

    impl Row {
        fn callback(selector: &'static str, epilogue: Epilogue) -> Self {
            Self {
                selector,
                kind: VisitKind::Callback,
                active: true,
                epilogue: Some(epilogue),
                cleanup: None,
            }
        }
    }

    #[derive(Debug, Default)]
    struct TestWorld {
        rows: BTreeMap<u16, Row>,
        receipts: Vec<ScheduleReceipt<&'static str>>,
        callbacks: Vec<(SourcePass, u16, u16, &'static str)>,
        phase_state: Vec<(SourcePass, PassPhase)>,
        epilogue_state: Vec<(u16, &'static str, CurrentOccupant<&'static str>)>,
        cleanup_state: Vec<(u16, CurrentOccupant<&'static str>)>,
    }

    impl TestWorld {
        fn insert(&mut self, slot: u16, row: Row) {
            self.rows.insert(slot, row);
        }

        fn active_signature(&self) -> Vec<(u16, &'static str, VisitKind)> {
            self.rows
                .iter()
                .filter(|(_, row)| row.active)
                .map(|(slot, row)| (*slot, row.selector, row.kind))
                .collect()
        }
    }

    impl SourceSchedulerWorld for TestWorld {
        type Selector = &'static str;
        type Epilogue = Epilogue;
        type Cleanup = Cleanup;
        type Progress = Vec<(u16, &'static str, VisitKind)>;
        type Error = std::convert::Infallible;

        fn phase(&mut self, pass: SourcePass, phase: PassPhase) -> Result<(), Self::Error> {
            self.phase_state.push((pass, phase));
            Ok(())
        }

        fn resolve(
            &self,
            _pass: SourcePass,
            _scan: u16,
            slot: u16,
        ) -> SlotResolution<Self::Selector, Self::Epilogue, Self::Cleanup> {
            let Some(row) = self.rows.get(&slot) else {
                return SlotResolution::Inactive;
            };
            if !row.active {
                return SlotResolution::Inactive;
            }
            if row.selector == "unknown" {
                return SlotResolution::Unknown {
                    selector: row.selector,
                };
            }
            SlotResolution::Eligible(VisitPlan {
                selector: row.selector,
                kind: row.kind,
                epilogue: row.epilogue,
                cleanup: row.cleanup,
            })
        }

        fn callback(
            &mut self,
            pass: SourcePass,
            scan: u16,
            slot: u16,
            selector: &Self::Selector,
        ) -> Result<(), Self::Error> {
            self.callbacks.push((pass, scan, slot, *selector));
            match *selector {
                "birth-seed" => {
                    self.insert(100, Row::callback("earlier", Epilogue::Deactivate));
                    self.insert(700, Row::callback("later", Epilogue::Deactivate));
                    self.insert(500, Row::callback("same", Epilogue::Deactivate));
                }
                "mutator" => {
                    self.rows.get_mut(&700).expect("later row exists").selector = "later-new";
                }
                _ => {}
            }
            Ok(())
        }

        fn epilogue(
            &mut self,
            _pass: SourcePass,
            _scan: u16,
            slot: u16,
            instruction: &Self::Epilogue,
            current: &CurrentOccupant<Self::Selector>,
        ) -> Result<(), Self::Error> {
            let original = self.rows.get(&slot).map_or("inactive", |row| row.selector);
            self.epilogue_state.push((slot, original, current.clone()));
            if *instruction == Epilogue::Deactivate {
                if let Some(row) = self.rows.get_mut(&slot) {
                    row.active = false;
                }
            }
            Ok(())
        }

        fn cleanup(
            &mut self,
            _pass: SourcePass,
            _scan: u16,
            slot: u16,
            _instruction: &Self::Cleanup,
            current: &CurrentOccupant<Self::Selector>,
        ) -> Result<(), Self::Error> {
            self.cleanup_state.push((slot, current.clone()));
            self.rows.remove(&slot);
            Ok(())
        }

        fn progress(&self, _pass: SourcePass) -> Self::Progress {
            self.active_signature()
        }

        fn record(&mut self, receipt: ScheduleReceipt<Self::Selector>) {
            self.receipts.push(receipt);
        }
    }

    #[test]
    fn cf10_phase_trace_matches_rng_prepass_rebuild_dispatch_epilogue_cleanup() {
        let mut world = TestWorld::default();
        let mut row = Row::callback("ordinary", Epilogue::Deactivate);
        row.cleanup = Some(Cleanup::Reclaim);
        world.insert(20, row);

        let stats = run_update_entities(&mut world).expect("literal update scan succeeds");
        assert_eq!(
            stats,
            ScheduleStats {
                scans: 1,
                eligible_visits: 1,
                callbacks: 1,
                epilogues: 1,
                cleanups: 1,
            }
        );
        assert_eq!(
            world.phase_state,
            vec![
                (SourcePass::UpdateEntities, PassPhase::RngPrepass),
                (SourcePass::UpdateEntities, PassPhase::FollowingPassCleanup,),
                (SourcePass::UpdateEntities, PassPhase::RebuildMemberships,),
                (SourcePass::UpdateEntities, PassPhase::PreDispatch),
                (SourcePass::UpdateEntities, PassPhase::PostDispatch),
            ]
        );
        let receipt_kinds = world
            .receipts
            .iter()
            .map(|receipt| match receipt {
                ScheduleReceipt::Phase { phase, .. } => match phase {
                    PassPhase::RngPrepass => "rng",
                    PassPhase::FollowingPassCleanup => "following-cleanup",
                    PassPhase::RebuildMemberships => "rebuild",
                    PassPhase::PreDispatch => "prepass",
                    PassPhase::PostDispatch => "postpass",
                },
                ScheduleReceipt::ScanStarted { .. } => "scan-start",
                ScheduleReceipt::SlotResolved { .. } => "resolve",
                ScheduleReceipt::CallbackCompleted { .. } => "callback",
                ScheduleReceipt::EpilogueCompleted { .. } => "epilogue",
                ScheduleReceipt::CleanupCompleted { .. } => "cleanup",
                ScheduleReceipt::ScanFinished { .. } => "scan-finish",
            })
            .collect::<Vec<_>>();
        assert_eq!(
            receipt_kinds,
            [
                "rng",
                "following-cleanup",
                "rebuild",
                "prepass",
                "scan-start",
                "resolve",
                "callback",
                "epilogue",
                "cleanup",
                "scan-finish",
                "postpass",
            ]
        );
        assert_eq!(
            world.cleanup_state,
            vec![(20, CurrentOccupant::Inactive)],
            "cleanup must observe epilogue state, not the pre-callback row"
        );
    }

    #[test]
    fn cf10_runagain_and_earlier_later_allocations_match_source() {
        let mut apply = TestWorld::default();
        apply.insert(500, Row::callback("birth-seed", Epilogue::Keep));
        let stats = run_apply_events(&mut apply, NonZeroU16::new(8).unwrap())
            .expect("ApplyEvents reaches a fixpoint");
        assert_eq!(stats.scans, 3, "two productive scans plus quiescence scan");
        assert_eq!(
            apply
                .callbacks
                .iter()
                .map(|(_, scan, slot, selector)| (*scan, *slot, *selector))
                .collect::<Vec<_>>(),
            [
                (0, 500, "birth-seed"),
                (0, 700, "later"),
                (1, 100, "earlier"),
                (1, 500, "same"),
            ],
            "ApplyEvents must reach a higher birth immediately and restart for earlier/same births"
        );
        assert!(
            apply.epilogue_state.iter().any(|(slot, _, current)| {
                *slot == 500
                    && *current
                        == CurrentOccupant::Known {
                            selector: "same",
                            kind: VisitKind::Callback,
                        }
            }),
            "the seed epilogue must resolve the self-slot replacement"
        );

        let mut update = TestWorld::default();
        update.insert(500, Row::callback("birth-seed", Epilogue::Keep));
        run_update_entities(&mut update).expect("first update succeeds");
        assert_eq!(
            update
                .callbacks
                .iter()
                .map(|(_, _, slot, selector)| (*slot, *selector))
                .collect::<Vec<_>>(),
            [(500, "birth-seed"), (700, "later")],
            "UpdateEntities must defer earlier and same-slot births"
        );
        run_update_entities(&mut update).expect("following update succeeds");
        assert_eq!(
            update
                .callbacks
                .iter()
                .map(|(_, _, slot, selector)| (*slot, *selector))
                .collect::<Vec<_>>(),
            [
                (500, "birth-seed"),
                (700, "later"),
                (100, "earlier"),
                (500, "same"),
            ]
        );
    }

    #[test]
    fn cf10_apply_terminal_visit_does_not_request_a_redundant_rescan() {
        let mut world = TestWorld::default();
        world.insert(
            20,
            Row {
                selector: "terminal",
                kind: VisitKind::Terminal,
                active: true,
                epilogue: None,
                cleanup: Some(Cleanup::Reclaim),
            },
        );

        let stats = run_apply_events(&mut world, NonZeroU16::new(8).unwrap())
            .expect("terminal-only ApplyEvents scan completes without runagain");
        assert_eq!(
            stats,
            ScheduleStats {
                scans: 1,
                eligible_visits: 1,
                callbacks: 0,
                epilogues: 0,
                cleanups: 1,
            }
        );
        assert!(world.rows.is_empty());
        assert!(world.callbacks.is_empty());
        assert_eq!(world.cleanup_state.len(), 1);
    }

    #[test]
    fn cf10_live_later_selector_mutation_uses_at_visit_resolution() {
        let mut world = TestWorld::default();
        world.insert(100, Row::callback("mutator", Epilogue::Deactivate));
        world.insert(700, Row::callback("later-old", Epilogue::Deactivate));

        run_update_entities(&mut world).expect("live selector update succeeds");
        assert_eq!(
            world
                .callbacks
                .iter()
                .map(|(_, _, slot, selector)| (*slot, *selector))
                .collect::<Vec<_>>(),
            [(100, "mutator"), (700, "later-new")],
            "the later selector must be resolved after the earlier callback mutates it"
        );
    }

    #[test]
    fn cf10_unknown_selector_faults_before_callback_or_epilogue() {
        let mut world = TestWorld::default();
        world.insert(50, Row::callback("unknown", Epilogue::Deactivate));

        let error = run_update_entities(&mut world).expect_err("unknown rows fail closed");
        assert_eq!(
            error,
            ScheduleError::UnknownDispatch {
                pass: SourcePass::UpdateEntities,
                scan: 0,
                slot: 50,
                selector: "unknown",
            }
        );
        assert!(world.callbacks.is_empty());
        assert!(world.epilogue_state.is_empty());
    }

    #[test]
    fn cf10_apply_retry_is_scheduler_owned_and_non_progress_is_bounded() {
        let mut world = TestWorld::default();
        world.insert(10, Row::callback("persistent", Epilogue::Keep));

        let error = run_apply_events(&mut world, NonZeroU16::new(8).unwrap())
            .expect_err("an eligible row that makes no progress cannot spin forever");
        assert_eq!(
            error,
            ScheduleError::ApplyNoProgress {
                scan: 0,
                eligible_visits: 1,
            }
        );
        assert_eq!(world.callbacks.len(), 1);
    }

    #[test]
    fn cf10_invalid_selector_cleanup_missing_row_fails_loudly() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 cleanup fixture loads");
        let mut session = inspection
            .level2_reset_session()
            .expect("qualified Level 2 cleanup session opens");
        let receipts_before = session.cf10_production_schedule_receipts();

        let error = session
            .cf10_probe_invalid_selector_cleanup(0)
            .expect_err("a missing Archer row cannot produce a successful cleanup receipt");
        assert!(error.contains("lost the live Archer child at source slot 0"));
        assert_eq!(session.cf10_production_schedule_receipts(), receipts_before);
    }

    #[test]
    fn cf10_registry_scheduler_preserves_omega() {
        assert!(OMEGA.contains("\"schema_version\": \"campaign-fold-omega-v1.1\""));
        assert!(OMEGA.contains("\"json_byte_identical\": true"));

        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 Omega fixture loads");
        let mut session = inspection
            .level2_reset_session()
            .expect("qualified Level 2 Omega session opens");
        let initial_hash = session.campaign_state_hash();
        let initial_terrain_hash = session.terrain_hash();
        let initial_rng = session.cf07_rng_observation().0;

        let first_evidence = session.step_outer_turn(PlayerCommand::default(), 0);
        let first_events = session.dispatch_events().to_vec();
        let first_hash = session.campaign_state_hash();
        let first_terrain_hash = session.terrain_hash();
        let first_rng = session.cf07_rng_observation().0;
        let first_receipts = session.cf10_production_schedule_receipts();
        assert!(
            !first_evidence.is_empty(),
            "production pass failed: {first_events:?}; receipts: {:?}",
            first_receipts
        );
        assert!(!first_events.is_empty());
        assert!(first_events.iter().all(|event| !matches!(
            event,
            crate::native_world::Level2DispatchEvent::Error { .. }
        )));
        assert_eq!(
            first_rng,
            initial_rng.wrapping_mul(9_377).wrapping_add(9_439),
            "ordinary speed-0 execution owns exactly one UpdateEntities RNG prepass and no ApplyEvents pass"
        );
        assert!(matches!(
            first_receipts.as_slice(),
            [
                ScheduleReceipt::Phase {
                    pass: SourcePass::UpdateEntities,
                    phase: PassPhase::RngPrepass,
                },
                ScheduleReceipt::Phase {
                    pass: SourcePass::UpdateEntities,
                    phase: PassPhase::FollowingPassCleanup,
                },
                ScheduleReceipt::Phase {
                    pass: SourcePass::UpdateEntities,
                    phase: PassPhase::RebuildMemberships,
                },
                ScheduleReceipt::Phase {
                    pass: SourcePass::UpdateEntities,
                    phase: PassPhase::PreDispatch,
                },
                ScheduleReceipt::ScanStarted {
                    pass: SourcePass::UpdateEntities,
                    scan: 0,
                },
                ..,
                ScheduleReceipt::ScanFinished {
                    pass: SourcePass::UpdateEntities,
                    scan: 0,
                    ..
                },
                ScheduleReceipt::Phase {
                    pass: SourcePass::UpdateEntities,
                    phase: PassPhase::PostDispatch,
                },
            ]
        ));

        let mut callback_visits = 0_usize;
        let mut scheduler_owned_player = None;
        let mut verified_delegate = None;
        for (index, receipt) in first_receipts.iter().enumerate() {
            let ScheduleReceipt::SlotResolved {
                slot,
                selector,
                kind: VisitKind::Callback,
                ..
            } = receipt
            else {
                continue;
            };
            callback_visits += 1;
            assert!(
                CAMPAIGN_BEHAVIOR_REGISTRY
                    .entry(selector.registry_key())
                    .is_some(),
                "production callback at source slot {slot} must be selected by the compiled registry"
            );
            assert!(matches!(
                first_receipts.get(index + 1),
                Some(ScheduleReceipt::CallbackCompleted {
                    slot: completed_slot,
                    selector: completed,
                    ..
                }) if completed_slot == slot && completed == selector
            ));
            assert!(matches!(
                first_receipts.get(index + 2),
                Some(ScheduleReceipt::EpilogueCompleted {
                    slot: completed_slot,
                    original,
                    ..
                }) if completed_slot == slot && original == selector
            ));

            match selector.registry_key().selector {
                BehaviorSelector::ClassAction {
                    class: 3,
                    action: 0,
                } => {
                    let current_phase = match first_receipts.get(index + 2) {
                        Some(ScheduleReceipt::EpilogueCompleted {
                            current: CurrentOccupant::Known { selector, .. },
                            ..
                        }) => selector.observed_phase(),
                        _ => None,
                    };
                    scheduler_owned_player =
                        Some((*slot, selector.observed_phase(), current_phase));
                }
                BehaviorSelector::ClassAction {
                    class: 10,
                    action: 52,
                } if verified_delegate.is_none() => {
                    let current_phase = match first_receipts.get(index + 2) {
                        Some(ScheduleReceipt::EpilogueCompleted {
                            current: CurrentOccupant::Known { selector, .. },
                            ..
                        }) => selector.observed_phase(),
                        _ => None,
                    };
                    verified_delegate = Some((selector.observed_phase(), current_phase));
                }
                _ => {}
            }
        }
        assert!(
            callback_visits > 0,
            "the production scan must dispatch real rows"
        );
        let (player_slot, before, current_before_epilogue) =
            scheduler_owned_player.expect("qualified player callback is present");
        assert_eq!(
            current_before_epilogue, before,
            "the retained player callback must not perform the scheduler-owned phase advance"
        );
        assert_eq!(
            session.cf10_live_source_phase(player_slot),
            before.map(|phase| phase.wrapping_add(1)),
            "the real adapter epilogue advances the re-resolved player exactly once"
        );
        let (before, current_before_epilogue) =
            verified_delegate.expect("qualified house callback is present");
        assert_eq!(
            current_before_epilogue,
            before.map(|phase| phase.wrapping_add(1)),
            "the adapter must verify the retained delegate-owned phase advance"
        );

        session.reset();
        assert_eq!(session.campaign_state_hash(), initial_hash);
        assert_eq!(session.terrain_hash(), initial_terrain_hash);
        assert!(session.dispatch_events().is_empty());
        assert_eq!(
            session.step_outer_turn(PlayerCommand::default(), 0),
            first_evidence
        );
        assert_eq!(session.dispatch_events(), first_events.as_slice());
        assert_eq!(session.campaign_state_hash(), first_hash);
        assert_eq!(session.terrain_hash(), first_terrain_hash);
        assert_eq!(session.cf07_rng_observation().0, first_rng);
        assert_eq!(session.cf10_production_schedule_receipts(), first_receipts);
    }

    #[test]
    fn cf10_production_null_callbacks_are_exact_and_mismatches_roll_back() {
        for null_action in [82_u8, 84_u8] {
            let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
                .expect("qualified Level 2 null-callback fixture loads");
            let mut session = inspection
                .level2_reset_session()
                .expect("qualified Level 2 null-callback session opens");
            session
                .allocate_level2_authored_model22(110)
                .expect("qualified Whirlwind parent allocates");
            let child_slot = session
                .cf10_first_model22_child_slot()
                .expect("qualified Whirlwind parent allocates its child chain");
            assert!(session.cf10_set_model22_child_action(child_slot, null_action));

            let evidence = session.step_outer_turn(PlayerCommand::default(), 0);
            assert!(!evidence.is_empty());
            assert!(
                session
                    .dispatch_events()
                    .iter()
                    .all(|event| !matches!(event, Level2DispatchEvent::Error { .. }))
            );
            let receipts = session.cf10_production_schedule_receipts();
            assert!(receipts.iter().any(|receipt| matches!(
                receipt,
                ScheduleReceipt::SlotResolved {
                    slot,
                    selector,
                    kind: VisitKind::NoCallback,
                    ..
                } if *slot == child_slot
                    && selector.registry_key().selector
                        == BehaviorSelector::ClassAction {
                            class: 10,
                            action: null_action,
                        }
            )));
            assert!(!receipts.iter().any(|receipt| matches!(
                receipt,
                ScheduleReceipt::CallbackCompleted { slot, .. }
                    | ScheduleReceipt::EpilogueCompleted { slot, .. }
                    if *slot == child_slot
            )));
        }

        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 mismatch fixture loads");
        let mut session = inspection
            .level2_reset_session()
            .expect("qualified Level 2 mismatch session opens");
        session
            .allocate_level2_authored_model22(110)
            .expect("qualified Whirlwind parent allocates");
        let child_slot = session
            .cf10_first_model22_child_slot()
            .expect("qualified Whirlwind parent allocates its child chain");
        assert!(session.cf10_set_model22_child_action(child_slot, 83));
        let mismatch_key = crate::campaign::spec::BehaviorKey {
            dispatch_site: crate::campaign::dispatch::DispatchSite::UpdateEntities.id(),
            selector: BehaviorSelector::ClassAction {
                class: 10,
                action: 83,
            },
        };
        assert!(
            CAMPAIGN_BEHAVIOR_REGISTRY.entry(&mismatch_key).is_some(),
            "the negative case must be a registered callback with the wrong carrier, not merely an unknown key"
        );
        let hash_before = session.campaign_state_hash();
        let rng_before = session.cf07_rng_observation();
        let evidence = session.step_outer_turn(PlayerCommand::default(), 0);

        assert!(evidence.is_empty());
        assert_eq!(session.campaign_state_hash(), hash_before);
        assert_eq!(session.cf07_rng_observation(), rng_before);
        assert!(matches!(
            session.dispatch_events(),
            [Level2DispatchEvent::Error {
                source_slot,
                message,
                ..
            }] if *source_slot == child_slot && message.contains("cannot consume")
        ));
        assert!(
            !session
                .cf10_production_schedule_receipts()
                .iter()
                .any(|receipt| matches!(
                    receipt,
                    ScheduleReceipt::CallbackCompleted { slot, .. }
                        | ScheduleReceipt::EpilogueCompleted { slot, .. }
                        if *slot == child_slot
                ))
        );
    }
}
