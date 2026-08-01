# Isomorphism cards

## L1-ISO-01 — canonicalize resident-mask callers

- Sites: `sim-core/src/level1_mission.rs` and its Rust/C API tests.
- Observable contract: every caller receives `required_resident_behavior_mask & !observed_resident_behavior_mask` with the same `u8` type and timing.
- Hidden differences: none; the old public method remains as a deprecated
  forwarding wrapper with no state access beyond its callee.
- Ordering/RNG/allocation/errors: none.
- ABI: no C ABI symbol or layout uses the Rust method name.
- Proof: focused resident tests, C API resident parity test, unchanged matrix validator inventory.
- Rollback: revert the single refactor commit.

## L1-ISO-02 — rejected distance-loop parameterization

- Site: `sim-core/src/lib.rs::journal_distance_loop`.
- Observable contract: sound 5 and sound 31 retain distinct source-audio call tags and distinct test-only `MissionAudioSite` identities; distance below 1,536 emits `PrepareSound`; distance at or above 1,536 emits `EndLoop`; entity index remains zero and all caller-provided fields are unchanged.
- Hidden differences: `audio_site` exists only under `cfg(test)`; production enum layout and events must be unchanged.
- Ordering: source call is marked before the event is appended, exactly once.
- RNG/allocation/errors: none; invalid sound IDs still panic before event construction.
- Proof result: runtime ordering tests passed, but the compiler-bound audio-site
  census failed because the compact form no longer exposed literal event-field
  bindings.
- Decision: rejected and restored before publication. The proof harness is an
  observable preservation contract, not incidental test structure.

## L1-ISO-03 — rejected after prototype

The validation-only arm merge was behaviorally isomorphic, but rustfmt produced no physical-line reduction. The prototype was reverted rather than claiming stylistic churn as simplification.
