# Level 1 isomorphic-cleanup baseline

Run: `2026-08-01-level1-pass-1`
Baseline commit: `c6d6dc84165207c571f74a4c370364b5ec4bc37f`
Scope: bounded private Level 1 Rust/Unity implementation and its human-facing status documents

## Frozen behavioral authority

| Evidence | Baseline |
|---|---:|
| Canonical conformance matrix | 182/182 Exact; 0 non-Exact; 29 structured exclusions |
| Rust workspace | 481 passed (4 atlas-registry, 70 sim-capi, 3 sim-cli integration, 400 sim-core, 4 sim-wasm) |
| Unity | EditMode composite 31/31; PlayMode 1/1 |
| PLAN29 acceptance | exactly one clean run A; run B is not permitted |
| Matrix SHA-256 | `2469237003fb977a447d0ef2496c03cf598cb55f3eae77b7d094f9fa258509c7` |
| Aggregate SHA-256 | `761a5872a25f0be2112e1810c556eee36f033471e40e3db9062483dcb413d089` |
| Closure receipts SHA-256 | `f456cd0f29355952bca39bd6349fbddd103b6fe849500094af3a750da837934b` |

The accepted Run-A receipts are reused as the pre-refactor baseline. Green milestone suites are not replayed between small edits; affected tests are run per candidate and the complete milestone gate is run once at the end.

## Structural baseline

| File | Lines |
|---|---:|
| `sim-core/src/level1_mission.rs` | 45,018 |
| `sim-capi/src/native_v2.rs` | 22,846 |
| `sim-core/src/lib.rs` | 5,996 |
| `sim-core/src/indexed_overlay.rs` | 3,480 |

The preserved all-target Clippy capture contains 654 unique `sim-core` pedantic diagnostics. That is a ceiling, not a Level 1 correctness failure. This pass may reduce it but must not increase it. Exact-width casts, source-shaped branches, distinct source-site arms, and large source-order state machines are not rewritten merely to silence a lint.
