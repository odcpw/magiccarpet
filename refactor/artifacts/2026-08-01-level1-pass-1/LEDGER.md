# Level 1 isomorphic cleanup ledger

## L1-ISO-01 — canonicalize resident-mask callers

- Lever: point active callers at the canonical resident-mask name.
- Compatibility: the old public Rust method remains as a deprecated forwarding
  alias, so downstream source compatibility and the C ABI remain unchanged.
- Behavior: all in-repo call sites now name the already-existing
  `remaining_resident_behavior_mask`; the expression, type, and timing remain
  unchanged.
- Focused proof: resident observation test and C API resident projection test
  pass 1/1 each.

## L1-ISO-02 — rejected distance-loop collapse

- Lever: parameterize the two sound IDs and two distance branches.
- Runtime behavior remained equivalent, but the compact form hid literal
  `audio_site` fields from the compiler-bound source census.
- Rejection proof:
  `tests::mission_audio_census_is_unique_complete_and_compiler_bound` failed
  while the prototype was present and passed after restoration.
- Decision: restore the source-shaped four arms. Proof transparency is part of
  the preservation contract.

## Reverted prototype

- L1-ISO-03 merged validation-only no-op arms but produced no net physical-line
  reduction after rustfmt. It was reverted.

## Current pass totals

- Accepted code candidates: 1 compatibility-safe naming cleanup.
- Rejected/deferred candidates: 6.
- Net production/test code LOC: +1 deprecation attribute; no runtime logic
  changed.
- Focused tests: 3 relevant tests passed; one additional overlay test passed
  during the reverted prototype.
- Formatting: `cargo fmt --all -- --check` passed.
- Final Rust inventory: 481/481 passed (400 `sim-core`, 70 `sim-capi`,
  4 `atlas-registry`, 3 `sim-cli`, 4 `sim-wasm`).
- Final matrix/view gate: 182/182 Exact, 29 exclusions, 481 resolving Cargo
  tests; a synthetic non-closed mutation was rejected before rendering.
- Fresh-eyes review: clean after the validator and Rust API findings were
  corrected.
