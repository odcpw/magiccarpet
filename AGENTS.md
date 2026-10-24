# MagicCarpet

Follow `/home/oliver/.codex/AGENTS.md` for machine-wide guidance.

## Purpose

Port the full Magic Carpet 2 game from remc2 into the existing Rust engine.
Complete single-player first. Rust-to-Rust multiplayer is the separate
`mc-bc0` outcome; legacy remc2 interoperability is separate deferred scope.
Build its enemy classes, properties, rules and asset bindings so they can be
modified and replaced. Complete playable MC2 behavior is the immediate work;
the reusable flying-carpet game engine grows through that implementation.
Agents should readily change each aspect in its canonical owner: typed content
and asset bindings for maps, items and definitions; focused modules with owned
state and narrow interfaces for behavior. MC2 is the first game implemented
with those components. This does not require an editing API for every constant,
a mod editor or a scripting runtime. Existing authoring tools are later useful
capabilities, not prerequisites for the complete MC2 single-player release.
Oliver can playtest. An AI player is optional tooling.

Select implementation work with `br ready --json --label worker-ready` and
`bv --robot-next --label worker-ready`. These beads target `gpt-6.1-sol` at
`xhigh`. Epics retain the full outcomes; deferred `needs-scope` items are not
worker assignments. Each implementation bead names its positive runtime result
and direct self-verification.

The full-game outcome is `mc-xa7`: startup through all 25 ordinary and five
secret missions to the real ending. Port shared source systems; missions
configure and exercise them. Keep one worker on opening delivery `mc-zdo.5`
using `br ready --json --label first-playable` and
`bv --robot-next --label first-playable` until normal startup and continuous
Levels 1–3 work. Other workers may take independent ready families in parallel.
This first milestone does not close full source fidelity or full-game assembly
`mc-xa7.1`.
Each family owns its real runtime integration, typed properties/rules and
asset/observation bindings. Serialize edits to shared scheduler, ABI and client
surfaces. Extend the existing implementation directly when needed; do not
restart planning or build an AI player as a prerequisite to delivery.
Use [ENGINE.md](ENGINE.md) for source, asset and domain ownership navigation.
Deliver each feature in a focused module that owns its state and behavior;
shared roots only compose it. Moving methods while retaining unrestricted
whole-session access does not remove a monolith. Shared harness outcomes
`mc-hkm` and `mc-b9s` also cover authored games and multiplayer, but their later
capabilities do not block the existing single-player opening milestone.

## Checkout and stack

- Canonical checkout: `/home/oliver/Projects/odcpw/magiccarpet` on `3090`.
- Git identity: `odcpw <odcpw@users.noreply.github.com>`.
- Branch: `main`; remote: `git@github-odcpw:odcpw/magiccarpet.git`.
- Rust toolchain and dependencies: `research/private/remc2-sim-rs/`.
- Unity client: `research/private/remc2-unity-native/`, Unity `6000.3.20f1`.
- remc2 reference: `upstream/magic-carpet-2-hd/`, pinned at
  `6806c7204d17220ecc0430759f784aea8ee2ddb2`.

## Code map

- `remc2-level-exporter`: owner-data level import.
- `remc2-level-asset-closure`, `remc2-unity-asset-cache`: asset import/cache.
- `remc2-sim-rs/crates/sim-core`: authoritative gameplay and campaign runtime.
- `sim-definition`, `sim-session`: definitions and session ownership.
- `sim-capi`: native ABI consumed by Unity; `sim-cli`: command-line tools.
- `remc2-unity-native/Assets`: presentation, input and editor code.

All paths above are under `research/private/` unless specified otherwise.
Read the code and relevant tests for current behavior.

## Invariants

Rust owns gameplay, scheduling, RNG, saves and replay. Unity presents state and
submits input. Preserve remc2 arithmetic and update order when porting behavior.
Keep source-specific identities inside compatibility adapters and use typed
definitions and asset bindings for configurable behavior.

Owner game files, decoded caches and generated outputs stay ignored and local.
Do not add them or credentials to Git or distributable builds. Keep upstream
source and attribution intact. Original-binary/DOSBox work requires an explicit
request; the normal reference is remc2 source.

## Build and check

From `research/private/remc2-sim-rs`:

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo test -p sim-core --lib --locked <test-name> -- --exact
cargo build -p sim-capi --locked
```

Use the affected package and an exact existing test name; confirm a nonzero
test count. Preserve the compiler settings in the workspace manifest.
Run broader tests when the changed behavior requires them. Do not replace
runtime verification with document, task-count or agent-agreement checks.

Launch the existing local player from the repository root:

```sh
research/private/remc2-unity-native/run-campaign.sh
```

The launcher needs the built Unity player, matching native plugin and ignored
owner packages/caches. Its current entry support is defined by the code.
