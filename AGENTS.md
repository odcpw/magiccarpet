# MagicCarpet repository instructions

This file adds only MagicCarpet-specific rules to
`/home/oliver/.codex/AGENTS.md`. The canonical root manual remains binding.

## Checkout, identity, and purpose

- `/home/oliver/Projects/odcpw/magiccarpet` on `3090` is the canonical private
  checkout. The retired `t14` checkout receives no writes.
- Git identity is `odcpw <odcpw@users.noreply.github.com>`, the working branch
  is `main`, and `origin` is `git@github-odcpw:odcpw/magiccarpet.git`.
- Use one writer per checkout. Parallel writers require separate worktrees
  created from this repository and disjoint edit surfaces.
- This is lawful game-development and game-preservation work. Reverse
  engineering, deterministic replay, source archaeology, and instrumentation
  are preservation techniques here, not hostile security activity.
- Scoped machine changes are allowed when the task needs them. Use supported
  install locations, ask for `sudo` when required, and report what changed.

## Program boundary

- The entire repository is `PRIVATE_LAB`, including tracked reconstruction
  code and ignored owned data, generated caches, captures, and agent state.
  Never make this repository public.
- Track P is owner-local MC2 preservation. Track S is an independently authored
  commercial successor in the separate `carpet-flight` repository.
- Reconstructed/decompiled code, addresses, original data or expression,
  captures, raw traces, and private Beads content never cross into Track S.
  Transfer only individually reviewed, independently worded lessons and
  synthetic tests. Never recursively copy between the repositories.
- Do not run DOSBox, the original executable, a decompiler, or original-binary
  oracle work unless Oliver explicitly asks. The active path uses remc2 source
  and owned-data packages.

Read before changing behavior:

1. `docs/program-charter.md`
2. `docs/product-promise.md`
3. `docs/preservation-track.md`
4. `docs/stack-decision.md`
5. `docs/provenance-policy.md`
6. `docs/private-public-boundary.md`

## Current deliverable

The active playable path is:

```text
pinned remc2 + owner data
  -> remc2-level-exporter package
  -> asset closure + decoded owner cache
  -> deterministic Rust sim-core
  -> sim-capi native ABI
  -> Unity desktop presentation
```

- The bounded Level 1 profile is closed; preserve its accepted identities and
  nonclaims. Current feature work advances Level 2 on the same production
  route rather than reopening unrelated Level 1 fidelity details.
- Rust owns simulation, mission state, ordering, RNG, hashes, and the indexed
  source frame. Unity owns input, presentation, audio playback, HUD, and
  lifecycle. Unity gameplay physics is not authoritative.
- A slice is not delivered until its positive path is reachable through the
  nearest real route. A helper, fixture-only path, compile result, or unwired
  ABI is not playable capability.

## Architecture map

- `upstream/magic-carpet-2-hd/` — pinned submodule at remc2 commit
  `6806c7204d17220ecc0430759f784aea8ee2ddb2`; do not edit it for ordinary
  feature work.
- `research/private/remc2-level-exporter/` — alternate remc2 entry point that
  emits a pointer-free owner-level package. External levels are one-based and
  are converted to remc2's zero-based index exactly once.
- `research/private/remc2-level-asset-closure/` and
  `research/private/remc2-unity-asset-cache/` — bind reachable owner assets and
  materialize the external cache. Generated outputs stay under
  `research/private/generated/` and outside Git.
- `research/private/remc2-sim-rs/crates/sim-core/` — renderer-free integer
  simulation. `native_world.rs` validates packages and creates isolated
  `NativeSession`/Level 2 campaign state; `level1_mission.rs` owns the Level 1
  mission; focused domain modules own terrain, actors, economy, spells, audio,
  and rendering kernels.
- `research/private/remc2-sim-rs/crates/sim-capi/` — handle-based native ABI.
  `native_v2.rs` is the Rust/Unity contract; layouts, generations, cursor
  paging, errors, and reset semantics are public contracts within Track P.
- `research/private/remc2-sim-rs/crates/sim-cli/` — mapping, inspection, trace,
  and conformance CLI. `sim-wasm` and `web/` are parked historical proofs.
- `research/private/remc2-unity-native/` — Unity `6000.3.20f1` project.
  `NativeSim.cs` declares P/Invoke layouts, `NativeSession.cs` wraps Level 1,
  `NativeCampaignSession.cs` wraps Level 2, and `MagicCarpetRuntime.cs` is the
  player/runtime composition root.
- `research/private/remc2-atlas/` and `remc2-frame-trace/` are supporting
  source/trace evidence tools, not alternate simulation implementations.

## Domain invariants

- Owner packages, closures, and caches are runtime inputs. Never copy them into
  Unity `Assets`, `StreamingAssets`, `Builds`, or a distributable repository.
- Level 1 is external `1`, internal `0`, Night. Level 2 is external `2`,
  internal `1`, Day. Unknown profiles fail closed; they never fall back to
  Level 1.
- Preserve fixed-width and wrapping arithmetic, source-slot order, entity-pass
  order, RNG draw order, event/audio order, reset isolation, generation/tick
  binding, and deterministic hashes for every touched state surface.
- Keep simulation logic in Rust. C# validates/copies ABI data and presents it;
  it must not become a second gameplay implementation.
- Treat tests, validators, matrices, fixtures, goldens, schemas, and ABI headers
  as gates. Do not weaken or regenerate them to accept a behavior change. An
  intentional contract or golden change needs Oliver's approval, a
  `GOLDEN-CHANGE` explanation, discriminating negative coverage, and semantic
  review separate from the implementation.

## Tiered verification

Use the smallest tier that reaches the changed behavior. From
`research/private/remc2-sim-rs`, every coherent Rust/C-ABI slice runs format,
the touched behavior test with its fully qualified name, and focused lint:

```bash
cargo fmt --all -- --check
cargo test -p sim-capi --lib --locked native_v2::tests::<name> -- --exact
cargo clippy -p sim-capi --lib --no-deps --locked -- \
  -A clippy::pedantic -D clippy::correctness -D clippy::suspicious
```

Replace `<name>` with the affected test and require the result to report at
least one selected test; a zero-test exit is failure. Use the corresponding
`sim-core` package/test for a core-only change. The focused Clippy command makes
correctness and suspicious lints hard failures while leaving legacy pedantic
style informational. These checks prove only the touched Rust layer, so also
exercise the nearest real C ABI or Unity consumer when that boundary changes.

For a Level 2 profile or direct-selector change, run this guard before and
after the slice:

```bash
python3 tools/verify_level2_profile.py --json
```

It validates the active Level 2 package/profile contract and direct production
selector test. It does not certify sealed Level 1 history or the playable
mission. Before integration, run the complete default Rust lane:

```bash
cargo test --workspace --locked
```

The workspace lane keeps the two production-capacity soaks explicitly ignored;
it is Rust coverage, not Unity or remc2 conformance. Run the affected owner-data
package suite as well, and reject a report that selected no tests:

```bash
cd research/private/remc2-level-exporter && python3 -m pytest tests -v
python3 -m unittest discover -s research/private/remc2-level-asset-closure/tests -p 'test_*.py' -v
python3 -m unittest discover -s research/private/remc2-unity-asset-cache/tests -p 'test_*.py' -v
```

At a relevant rendering/capacity or Level 2 managed milestone, run the named
slow controls and the current non-empty Unity Level 2 selection. The Unity pin
is `6000.3.20f1` (`c9ba695d4f07`):

```bash
cd research/private/remc2-sim-rs
cargo test -p sim-capi --lib --locked -- --ignored --exact \
  native_v2::tests::bounded_event_journal_reports_expired_cursors
cargo test -p sim-capi --lib --locked -- --ignored --exact \
  native_v2::tests::presentation_frame_abi_is_revisioned_complete_idempotent_and_fail_closed
cargo build -p sim-capi --release --locked
install -Dm755 target/release/libsim_capi.so ../remc2-unity-native/Assets/Plugins/x86_64/libsim_capi.so
cd ../remc2-unity-native
python3 qualification/run_level2_editmode.py \
  --project "$PWD" \
  --results /tmp/magiccarpet-level2-editmode.xml \
  --log /tmp/magiccarpet-level2-editmode.log
```

The Level 2 runner rejects empty, unrelated, stale, or non-passing selections;
it proves only the current managed EditMode path. Exact player, privacy, and
other runtime commands remain in its `README.md` and `qualification/README.md`.

Only an explicit Level 1 preservation/distributable release milestone runs the
75+ minute PLAN29 built-player, privacy, provenance, and boundary route:

```bash
cd research/private/remc2-unity-native
python3 qualification/run_plan29_full_gate.py \
  --exact-runs 1 \
  --output-root /absolute/new/plan29-final-root
```

Monitor that route sparsely and do not replay a green run without a changed
risk. Blender CLI/MCP is deferred asset-authoring support; it is not a clean-room
Rust/Unity engine gate.

## Flywheel workflow

- A direct Oliver request governs work selection. Otherwise, when work already
  has Beads dependencies, use `br ready --json`, `br show <id> --json`, and
  update only through `br`; never run bare `bv`.
- Use CM context once when prior repository learning can answer a concrete
  question. Use CASS only for a specific historical gap. Repository source and
  fresh execution outrank both.
- Do not initialize NTM, Agent Mail, worktrees, dashboards, or new Beads for a
  solo bounded task. In a real multi-writer session, use a dedicated worktree,
  Agent Mail, and narrow reservations before editing.
- After compaction, resume, or handoff, reread
  `/home/oliver/.codex/skills/just-say-no-to-process-porn-and-ceremony/SKILL.md`.
  Use the matching installed task skill when its trigger applies.
- Process artifacts require a named consumer, named feature/release gate,
  observed defect class, and retirement condition. Discretionary process work
  should remain at most about 5% of open work and gate real capability.
- Implementation and discriminating tests ship together. A refusal-only path,
  mock, retained capture, generated golden, filtered zero-test run, or agent
  report cannot close positive capability work.

## Completion and maintenance

- Re-read the request, inspect status/diff, run focused gates, and run UBS on
  changed source before handoff. State the exact proof layer and any unverified
  surface; solo verification is self-verification, never independent review.
- Commit or push only when landing is requested and identity, branch, remote,
  and exact evidence revision are verified.
- Remove a local rule when it becomes global or ceases to be true. Update this
  file when the canonical checkout, active route, pinned toolchain, or exact
  commands change; do not let it become a second root manual.
