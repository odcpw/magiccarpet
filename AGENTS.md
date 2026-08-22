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

## Binding direction

Read before behavior or architecture work:

1. `docs/program-charter.md`
2. `docs/product-promise.md`
3. `docs/modular-engine-game-module-contract.md`
4. `docs/stack-decision.md`
5. `docs/private-public-boundary.md`
6. `docs/provenance-policy.md`
7. `docs/carpet-flight-engine-plan.md` while recovery remains active

Carpet Flight is one private remc2-derived Rust/Unity engine in this repository.
`MC2-owned` is the first package; later custom/original packages reuse the same
engine and semantic authoring API. Rust is authoritative. Unity is the primary
player/editor and future OpenXR shell. Three.js is an inspector and optional
client. Separate successor engines/repositories, clean-slate C# simulation,
browser-first authority and Track-P/Track-S transfer are retired premises.

Current priority is API-first extraction around the successful Level 1 route.
Before broad later-level work, establish the neutral Rust domain session,
package/definition, semantic transaction, client and conformance contracts and
the `MC2-owned` plus `custom-proof` gate. Existing Level 2 work is retained but
does not outrank this dependency.

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
- `research/private/remc2-sim-rs/crates/sim-capi/` — versioned transport;
  current Level 1/2 public shapes are compatibility surfaces, not the neutral
  domain contract.
- `research/private/remc2-sim-rs/crates/sim-cli/` — headless harness base.
- `research/private/remc2-unity-native/` — Unity `6000.3.20f1` client.
- `sim-wasm`/`web` are parked proofs; Three.js must consume the new contract.

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
- Before handoff, re-read the request, inspect status/diff, run proportionate
  focused gates and UBS on changed source when applicable. State exact proof
  layer and unverified surface. Commit/push only when requested.
