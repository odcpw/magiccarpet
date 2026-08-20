//! Internal campaign lifecycle seam for the sealed Level 1 session.
//!
//! This is static adapter dispatch only. It deliberately adds no public ABI,
//! runtime registry, descriptor variant, or alternate session state.

use std::{marker::PhantomData, path::Path};

use sim_core::{PlayerCommand, native_world::NativeWorld};

use super::{
    AbiError, AbiResult, MC_STATUS_INVALID_ARGUMENT, MC_STATUS_IO_ERROR,
    MC_STATUS_VALIDATION_FAILED, McSessionSnapshotV2, SessionRecord,
};

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

impl SealedLevel1Adapter {
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
    use std::path::PathBuf;

    use super::*;

    fn fixture_paths() -> (PathBuf, PathBuf) {
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        (
            private.join("generated/level1-export-m"),
            private.join("generated/level1-asset-closure.json"),
        )
    }

    fn open_fixture() -> SessionRecord {
        let (package_dir, closure_path) = fixture_paths();
        CampaignFacade::<SealedLevel1Adapter>::open(
            SealedLevel1Adapter::KEY,
            InternalCampaignOpen {
                package_dir: &package_dir,
                closure_path: Some(&closure_path),
                owner_cache: None,
            },
        )
        .expect("sealed Level 1 opens through the internal campaign facade")
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
