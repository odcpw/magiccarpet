//! Static campaign facade for the existing Level 2 session route.
//!
//! CF-01 moves the orchestration implementation into a child of
//! `native_world` while this module preserves a short, crate-level campaign
//! entry point for later fold leaves.  It introduces no carrier, descriptor,
//! registry, scheduler, or ABI semantics.

pub mod dispatch;
#[doc(hidden)]
pub mod source_arena;
pub mod source_facts;
pub mod spec;
pub mod state;

pub use crate::native_world::Level2ResetSession;

#[cfg(test)]
mod tests {
    use super::Level2ResetSession;
    use crate::{
        PlayerCommand,
        native_world::{Level2NyphurDispatchCall, NativeWorld},
    };
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
