# CF-00 Omega seal and v1.1 reseal

This directory is the evidence-only input seal for
`mc-cf00-seal-fold-inputs-omega-iplbq`. It freezes the pre-fold observation
alphabet and the accepted receipts at one explicit evidence revision. It does
not change production Rust, the C ABI, Unity, the exporter, owner packages,
goldens, denominators, or remc2, and it never runs the original executable.

## Evidence boundary

The sealed superproject revision is
`462b438d41f1d39ce4b4a1ae24f892dc3e04cafe`; remc2 remains pinned at
`6806c7204d17220ecc0430759f784aea8ee2ddb2`. The dirty parallel behavior
surfaces in `native_world.rs` and `native_world/level2_nyphur.rs` are excluded:
Omega seals only the committed source at that revision. Open Level 2 behavior
beads likewise remain outside the seal. The JSON lists the closed
source-pinned transition leaves with their full commit, ordinary producer, and
following-pass consumer. Aggregate beads are represented once by their leaf
receipts.

One closed tracker receipt is explicitly not sealed: `mc-hgk9.1` is recorded at
`932454914ab36262d192d13a24278d2f2f714463`, but that commit is not an ancestor
of the chosen evidence revision. It remains a parallel-boundary receipt rather
than being silently copied into the candidate. The open scheduler/RNG receipt
`mc-yawx.1.6` is also outside the closed set.

## Prerequisites

The canonical JSON records exact full commits and Git-object SHA-256 values for
every prerequisite:

| Receipt | Commit | Sealed input |
|---|---|---|
| `mc-campaign-fold-contract-r78sq` | `462b438d` | adopted fold graph and boundary |
| `mc-campaign-api-research-iaa85` | `3a73019c` | seam/API contract |
| `mc-fold0-level1-facade-uf827` | `0f43caf6` (accepted at `be739f7b`) | sealed Level 1 facade |
| `mc-campaign-rw-inventory-sh828` | `03723bd4` | method/schema only; checked hash rejected as stale |
| `mc-campaign-state-census-dxvvd` | `7bf3862` | 151 outer, 57 economy, 89 Nyphur fields; 127 retained pairs, 1 retired |
| `mc-campaign-registry-probe-udemg` | `0a0881f0` | 17 class rows, 486 source keys, 127 matches, 359 source-only |
| `mc-iol7n` | `00520df3` | Levels 3–24, 22/22 byte-identical realm twins |
| `mc-pkg-v5-no-collision-ugou9` | `6e3aeb70` | explicit-empty typed collision member and zero-count guard |

## Fresh M1 regeneration

The stale checked M1 artifact hashes `3e9da728…` and `691649822d…` are negative
fixtures only. From a clean detached worktree at the evidence revision, the
M1 builder was run into two fresh ignored roots:

```text
research/private/generated/mc-cf00-omega-a-462b
research/private/generated/mc-cf00-omega-b-462b
```

Both JSON and Markdown outputs were byte-identical. The accepted fresh M1 JSON
SHA-256 is `aad97175330bfdef21b09e3e164f054d2b6df59f66bd7c449f02e3f633514684`;
the Markdown SHA-256 is
`4e19cf3b3e9f0548b75ac97ce2cf3d7b82d57efdba3c9007bc51f11115470b4a`.
The regenerated inventory contains 54 transitions, 14 route kinds, and 224
open questions, and declares the exact Rust evidence revision.

## Omega builder reproducibility

The Omega builder was run twice in fresh ignored roots
`mc-cf00-omega-build-a3-462b` and `mc-cf00-omega-build-b3-462b`. Their JSON
outputs were byte-identical at SHA-256
`8aa245f6dd61aef07dc4481a6a4bfef069b6d4d6d243188379681837ad57545f`.
The committed canonical inventory is byte-identical to that pair at SHA-256
`8aa245f6dd61aef07dc4481a6a4bfef069b6d4d6d243188379681837ad57545f`.
Generated package/data roots remain ignored and local.

## v1.1 reseal and complete delta

The v1 artifact above remains unchanged as the audit baseline. The v1.1
reseal was measured at committed evidence revision
`656416b62fb6c859e842c3cb81cccaa38ac2df7f`, which includes the landed
`PreparedSound` sim-capi arm (`mc-yawx.2.2`). Fresh M1 runs at that revision
were byte-identical at JSON SHA-256
`7986b9a2935d160f40c22fae1250f7bfa9abe9ede81529832ed35ce148470383` and
Markdown SHA-256
`91b385359a70c78ab72ad011e0f6f5de827f334e1dcbe50283c1deb8565fc6ce`.
They contain 54 transitions, 14 route kinds, and 229 open questions. The
Omega builder was then run twice in fresh ignored roots; both v1.1 outputs
were byte-identical at SHA-256
`10d537151083d7fe0d2dae2d7409a3ef0fd5e42f2ddf4756e4d86812922aeef2`.
The tracked `campaign-fold-omega-v1.1.json` is that receipt; the v1 JSON is
retained separately for audit.

The denominator change is exactly `224 -> 229`. Every added question has both
an introducing receipt and the CF-01 path-relocation receipt; no open question
is absorbed without a row:

| New question | Transition | Delta | Introduced by | Relocated by |
|---|---|---:|---|---|
| `nyphur-possession-holder` (`level2.rs:1541`) | `slot-kind-11` | +1 | `mc-1g06.1` / `0eb2628d3c942a1c9844a5311359356e8c074eba` | `mc-cf01-pure-campaign-module-extraction-19t39` / `b0537fdf7491e782ca727896677ff66f90e8cf93` |
| `nyphur-fireball-holder` (`level2.rs:1380`) | `slot-kind-13` | +1 | `mc-1g06.1` / `0eb2628d3c942a1c9844a5311359356e8c074eba` | `mc-cf01-pure-campaign-module-extraction-19t39` / `b0537fdf7491e782ca727896677ff66f90e8cf93` |
| `group31-tree-source` (`level2.rs:3905`) | `source-prepass-011-activate_level2_authored_group31` | +1 | `mc-l2-33a6-tree-producer-7pclk` / `d1440cd8e0d05783135525c465f789f8046a4157` | `mc-cf01-pure-campaign-module-extraction-19t39` / `b0537fdf7491e782ca727896677ff66f90e8cf93` |
| `tree-callback-source` (`level2.rs:6192`) | `same-pass-epilogue-006-step_level2_dynamic_arena` | +1 | `mc-l2-33a6-tree-producer-7pclk` / `d1440cd8e0d05783135525c465f789f8046a4157` | `mc-cf01-pure-campaign-module-extraction-19t39` / `b0537fdf7491e782ca727896677ff66f90e8cf93` |
| `tree-callback-row` (`level2.rs:6222`) | `same-pass-epilogue-006-step_level2_dynamic_arena` | +1 | `mc-l2-33a6-tree-producer-7pclk` / `d1440cd8e0d05783135525c465f789f8046a4157` | `mc-cf01-pure-campaign-module-extraction-19t39` / `b0537fdf7491e782ca727896677ff66f90e8cf93` |

The intervening landed receipts `mc-yawx.1.6` (`10d0847e`),
`mc-yawx.1.8` (`f86e1141`, `ca77a1ce`), `mc-yawx.1.5` (`3be52495`),
`mc-yawx.2.2` (`fc3e1821`, `656416b6`), and `mc-1g06.1`'s later boundary
receipts (`73e1a09e`, `8968e761`) were measured and introduced no additional
open-question denominator drift. The `4590279e` and `4a638721` receipts are
already ancestors of the v1 boundary, so they are not post-v1 additions.
This is the explicit boundary: future receipts require a new mapped row or
must be reported as unmapped drift.

The v1.1 receipt is evidence-only. It changes no production Rust, C ABI,
Unity, exporter, owner package, or original-executable behavior. Its negative
suite additionally plants and rejects an unmapped open question, alongside
the v1 missing-state-cell, missing-transition, ordered-effect,
missing-capability, changed-revision, trace-difference, and stale-M1 cases.

## Omega alphabet and capability diff

The inventory covers authoritative state/hash, scheduler phase and slot
rescan, ordered RNG, allocation/reclaim/generation/topology, ordered effect
and audio rows with cursor binding, reset/persistent state/replay,
package/profile/source-fact preflight, C ABI transaction/layout/handle/error
receipts, and Unity-visible frame/journal outputs without Unity authority.
For every external level 3–24 it retains the class/action/model/subtype tuple
diff, objective-form diff, map-type gate, required-source-fact diff, package
identity, and implementation-capability comparison. The aggregate is 22
byte-identical twins across Day/Night/Cave, 3,179 distinct tuples, 5,978
active entities, and 12,703 authored nonzero rows.

The Python and Rust conformance tests plant and reject missing state, missing
transition, dropped/reordered ordered effect, missing capability, changed
revision, trace-on receipt difference, and stale-M1 cases. The trace test
checks that the sealed enabled and disabled receipt vectors are equal; no
runtime observer was added in this evidence-only bead, so this is not a claim
of a new production tracing facility.

## Acceptance commands

Run from `research/private/remc2-sim-rs` unless noted:

```bash
cargo test -p sim-capi --test campaign_fold_omega --locked -- --nocapture
python3 -m unittest discover -s level2-conformance/fold -p 'test_*.py' -v
cargo test -p sim-capi --lib --locked native_v2::campaign_facade::tests::sealed_level1_open_step_reset_replays_existing_hashes -- --exact
python3 tools/validate_level1_matrix.py --require-closed --json
cargo fmt --all -- --check
git diff --check
```

These tests are integrity gates for CF-00 and do not close Level 2, prove
later-realm playability, authorize a fold implementation, or retire the sealed
Level 1 path. Later CF leaves retire this receipt only after citing it and
reproducing its Omega observations at their candidate revision.

The shorter bead-supplied filter `native_v2::tests::...` was discovered first
and selected zero tests; it was rejected rather than counted as green. The
fully qualified `native_v2::campaign_facade::tests::...` path above selected
and passed one test.
