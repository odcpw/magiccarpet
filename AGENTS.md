# MagicCarpet repository instructions

This file adds only MagicCarpet-specific rules to
`/home/oliver/.codex/AGENTS.md`; the root manual remains binding.

## Checkout and purpose

- Canonical private checkout: `/home/oliver/Projects/odcpw/magiccarpet` on
  `3090`. Never write to the retired `t14` checkout.
- Git identity: `odcpw <odcpw@users.noreply.github.com>`; branch `main`; remote
  `git@github-odcpw:odcpw/magiccarpet.git`.
- Shared `main` is the default. Multiple writers require Bead claims, Agent
  Mail registration and exclusive narrow reservations. Worktrees need a named
  isolation reason.
- This repository is lawful private game-development and preservation work.
  Reverse engineering, replay and source archaeology are compatibility tools,
  not hostile-security work.

Build one Rust-authoritative Carpet Flight engine whose first real game package
is private `MC2-owned`; packages supply semantic game/world definitions and the
engine supplies reusable deterministic capabilities. Unity is the primary
player/editor, while humans and AI modify games through the same typed semantic
API—never through a Level-N engine, raw remc2 identity or client-side gameplay.

## Binding direction

Read before behavior or architecture work:

1. `docs/program-charter.md`
2. `docs/product-promise.md`
3. `docs/modular-engine-game-module-contract.md`
4. `docs/agent-addressable-game-architecture.md`
5. `docs/stack-decision.md`
6. `docs/private-public-boundary.md`
7. `docs/provenance-policy.md`

`docs/carpet-flight-engine-plan.md` is the completed foundation plan and dated
evidence index; it no longer selects current work.

Carpet Flight is one private remc2-derived Rust/Unity engine in this repository.
`MC2-owned` is the first package; later custom/original packages reuse the same
engine and semantic authoring API. Rust is authoritative. Unity is the primary
player/editor and future OpenXR shell. Three.js is an inspector and optional
client. Separate successor engines/repositories, clean-slate C# simulation,
browser-first authority and Track-P/Track-S transfer are retired premises.

Current priority is the package-derived semantic definition for real Levels 1,
2 and 3, then binding it through the existing Rust runtime. Level-specific
compatibility work is retained but does not define the public domain model.

Commercial distribution is a later exact-artifact gate. Owner-local MC2 play
and private compatibility work do not wait for it.

## Active route and important files

```text
pinned remc2 + owner data
  -> remc2-level-exporter
  -> asset closure + decoded owner cache
  -> deterministic Rust sim-core
  -> sim-capi native ABI
  -> Unity desktop client
```

- `upstream/magic-carpet-2-hd/` — pinned remc2 commit
  `6806c7204d17220ecc0430759f784aea8ee2ddb2`; normally read-only.
- `research/private/remc2-level-exporter/` — pointer-free owner-level package;
  convert external one-based to remc2 zero-based level exactly once.
- `research/private/remc2-level-asset-closure/` and
  `remc2-unity-asset-cache/` — owner asset closure and deterministic cache.
- `research/private/remc2-sim-rs/crates/sim-core/` — gameplay authority.
  `campaign/mc2_level_definition.rs` is the private MC2-number-to-semantic
  adapter; the production arena and scheduler remain here.
- `research/private/remc2-sim-rs/crates/sim-definition/` — immutable game/level
  definitions, semantic vocabulary, capabilities and authoring contracts.
- `research/private/remc2-sim-rs/crates/sim-session/` — definition-bound
  `GameSession` ownership and the neutral adapter/backend contract.
- `research/private/remc2-sim-rs/crates/sim-capi/` — versioned transport;
  current Level 1/2 public shapes are compatibility surfaces, not the neutral
  domain contract.
- `research/private/remc2-sim-rs/crates/sim-cli/` — headless harness base.
- `research/private/remc2-unity-native/` — Unity `6000.3.20f1` client.
- `sim-wasm`/`web` are parked proofs; Three.js must consume the new contract.

## Semantic level architecture

- [`docs/agent-addressable-game-architecture.md`](docs/agent-addressable-game-architecture.md)
  is the binding definition/runtime/authoring contract and first-three-level
  pause milestone. Apply it to every domain, not only creatures.
- Keep raw remc2 identities in the MC2 adapter, one Rust runtime authority, and
  package-present semantic content only. Definition support is not behavior
  support; deliver the complete catalog -> package binding -> runtime lifecycle
  -> downstream consumer chain.
- Rename a touched `Level2*`/`class5`/`family*` type only when its ownership is
  genuinely shared; do not add aliases or launch a cosmetic global rename.
- For crate-boundary work, use the installed
  `de-monolithize-your-codebase-isomorphically` and
  `simplify-and-refactor-code-isomorphically` skills; preserve behavior, the
  compiler mitigation and focused owning-crate verification described by the
  architecture contract.

## Invariants

- Owner packages, caches, captures and generated outputs remain ignored/external
  and never enter Unity `Assets`, `StreamingAssets` or distributable builds.
- Level 1 is external `1`, internal `0`, Night; Level 2 is external `2`,
  internal `1`, Day. Unknown profiles fail closed.
- Rust solely owns live gameplay state, definitions, ordering, RNG, hashes and
  saves/replays. C#/TypeScript validate/copy/present; no client gameplay state.
- Presentation polling must be observational. Rust owns any deterministic
  notification/presentation lifecycle needed by later steps.
- Preserve fixed-width/wrapping arithmetic, source-slot and entity-pass order,
  RNG draw order, event/audio order, reset isolation and generation/tick
  binding.
- A remc2 behavior identity includes producer/constructor, runtime scheduler
  and dispatch key, handler, same-pass epilogue/hide, following-pass
  cleanup/reclaim, authoritative carrier and downstream consumer.
- Source IDs stay inside the MC2 adapter. New public APIs use semantic IDs and
  no Level-N authority.
- Tests, validators, schemas, fixtures, goldens and ABI headers are gates. Never
  weaken/regenerate them to accept a behavior change. Intentional golden or
  contract changes require Oliver's approval, a `GOLDEN-CHANGE` explanation,
  discriminating negative coverage and separate semantic review.

Do not run DOSBox, the original executable, a decompiler or original-binary
oracle work unless Oliver explicitly asks. The normal path uses remc2 source and
owned-data packages.

## Focused Rust verification

From `research/private/remc2-sim-rs`, select the exact affected test and require
at least one test:

```bash
cargo fmt --all -- --check
cargo test -p sim-capi --lib --locked native_v2::tests::<name> -- --exact
cargo clippy -p sim-capi --lib --no-deps --locked -- \
  -A clippy::pedantic -D clippy::correctness -D clippy::suspicious
```

Use the corresponding `sim-core` package/test for core-only changes. Discover a
filter once with `cargo test ... -- --list | rg -F '<name>: test'`; zero selected
tests fail. For Level 2 profile/direct-selector changes run:

```bash
python3 tools/verify_level2_profile.py --json
```

Before Rust integration:

```bash
cargo test --workspace --locked
```

Run affected owner-data tool suites as well:

```bash
cd research/private/remc2-level-exporter && python3 -m pytest tests -v
python3 -m unittest discover -s research/private/remc2-level-asset-closure/tests -p 'test_*.py' -v
python3 -m unittest discover -s research/private/remc2-unity-asset-cache/tests -p 'test_*.py' -v
```

At a relevant native/Unity milestone, build/install the exact plugin and use the
current non-empty qualification runner documented under
`research/private/remc2-unity-native/qualification/`. Verify ignored owner
inputs and plugin provenance first; never manufacture substitutes or call a
`DllNotFoundException` behavior evidence. The 75+ minute PLAN29 route runs only
for an explicit preservation/distribution milestone.

Expensive unchanged gates run once per relevant candidate and verification
owner. Retain output/exit status and resume yielded commands instead of
restarting them.

## Work selection and completion

- **Execution-first recovery after compaction.** Reread only this repository
  file, recover the newest user goal, inspect Git/dirty state and the active
  diff or running command, then resume the exact implementation where it
  stopped. Do not reread skills, architecture documents, old reviews or prior
  agent reports unless a concrete missing fact in the current code requires
  one of them. Do not restart work selection, archaeology, design comparison,
  threat modeling or review after compaction. The default next command is the
  compiler, the focused owning test, or the edit needed to fix its failure.
- **Ship and debug; do not review-loop.** Once an implementation direction is
  consistent with the current user goal and repository architecture, carry it
  through code, compilation and a real focused test before reconsidering the
  design. A failing compiler/test/runtime observation is the normal debugging
  loop, not a trigger for another architecture round. Reopen the design only
  when current execution produces evidence that the chosen seam cannot satisfy
  the goal or would create duplicate authority/data loss.
- Do not dispatch review, archaeology, contract, gate or adversarial subagents
  unless Oliver explicitly requests them. One implementation owner is the
  default. Agent agreement is not a delivery requirement and repeated GO/NO-GO
  rounds are prohibited when the relevant code or evidence has not changed.
- Spend the large majority of a work block implementing and debugging working
  capability. Planning, archaeology, documentation and review must answer a
  concrete question that blocks the next code/test step; stop them immediately
  once that question is answered. Do not produce readiness matrices, scoring
  rounds or alternative-slice rankings while an aligned implementation can be
  advanced.
- Treat the owner package/campaign definition as the level capability census:
  test only behavior families actually reachable from that package, plus the
  shared invariants they exercise; never build absent-content matrices. The
  qualified package and pinned remc2 outrank guides and reports. Read enough to
  locate the authoritative carrier and first real consumer, then implement and
  learn from execution. Deepen source archaeology only when a concrete failed
  observation cannot be explained from current code.
- New capability needs one focused owning positive test. Add a nearest-negative
  assertion when it cheaply discriminates a plausible partial implementation;
  do not hold working code hostage to speculative gate design, broad matrices,
  omnibus tests or repeated test-review rounds. State remaining scope plainly
  in the handoff instead of engineering exhaustive No-Claim ceremony.
- A yielded test process is still the same run: resume or inspect it instead of
  starting another, and never infer success because it disappeared. If its exit
  result is irretrievably lost, rerun that exact test once and say why.
- Put new tests in the owning behavior/module file with a descriptive name.
  Never add to `misc.rs`; extend a test only while it remains one causal chain.
- Direct Oliver requests govern. Otherwise use `br ready --json` only after the
  recovery graph is reconciled; the pre-recovery graph selects obsolete work.
- A Bead is worker-ready only when inputs, owned surfaces, exact acceptance,
  temporal production chain, downstream consumer and focused verification are
  pinned. Helpers, setters, fixtures and refusal-only paths cannot close a
  positive capability.
- Preserve unexpected dirty work. The current recovery baseline includes
  pre-existing `.beads/issues.jsonl` changes for `mc-6dqd.1.1`, `mc-jmcp` and
  `mc-uptb.1`; do not overwrite their late evidence.
- Do not initialize NTM, Agent Mail, worktrees or new process artifacts for solo
  work. For justified concurrent writers, follow the root coordination rules.
- Before handoff, inspect status/diff and run the focused owning test plus
  proportionate compile/format checks. Run broader suites or scanners only
  when the changed surface warrants them or Oliver asks; do not let noisy
  legacy findings displace debugging the candidate. State what ran and what
  remains unverified. Commit/push only when requested.
