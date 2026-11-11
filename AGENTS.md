# MagicCarpet

Follow `/home/oliver/.codex/AGENTS.md` for machine-wide guidance.

Read [WORK_PLAN.md](WORK_PLAN.md) before every assignment, follow-up, resumed
session or orchestration tick, and before a completion handoff. It is the
canonical current delivery plan; keep prompts short and refer to it.

## Gameplay capabilities are the unit of work

Keep the current plan and its full acceptance. Execute it by completing connected
gameplay capabilities, with these priorities and ownership rules:

- Prioritize the known failing shared paths: Castle, Beyond Sight, Alliance
  ownership and summoned-creature production. Resolve their missing consumers
  before adding isolated implementations elsewhere.
- Give each path one named accountable owner through real acquisition/spawn →
  scheduling → interaction → consequences → cleanup → presentation. That owner
  stays with consumer failures until the connected capability works. Integrators
  actively implement required shared joins with the owner.
- Trace remc2's actual authoritative state, arithmetic and update order, including
  same-pass tails and following-pass reclaim. Fix canonical shared behavior;
  missions configure and exercise it. Do not patch individual missions around a
  shared defect.
- Every assignment names a concrete gameplay outcome, owned surface, integration
  owner and decisive way to check the result. Representative spell–creature
  checks exercise the shared boundary; add focused checks for genuine exceptions.
- Keep partial work attached to its capability. Do not accumulate leaf handoffs
  or count a callback, constructor, trace or passing leaf test as delivery.
- Parallelize independent ownership. When integration blocks workers, redirect
  available capacity to the concrete blocker and serialize shared-file edits.
- Report **implemented**, **verified**, **committed** and **installed** separately,
  with the relevant revision and evidence. Installed means matching native/player
  artifacts reached the actual client. Keep unfinished acceptance open.

No new planning rounds, test-count targets or assertion weakening. Deliver
complete, installed capabilities while preserving unfinished work and scope.

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
`bv --robot-next --label worker-ready`. Oliver's current fleet choice is
`gpt-6.1-terra` at `xhigh` for workers and `gpt-6.1-sol` at `xhigh` for integrators;
this supersedes older model text in beads. Epics retain the full outcomes;
deferred `needs-scope` items are not
worker assignments. Each implementation bead names its positive runtime result
and direct self-verification.

The full-game outcome is `mc-xa7`: startup through all 25 ordinary and five
secret missions to the real ending. Port shared source systems; missions
configure and exercise them. Keep one worker on opening delivery `mc-zdo.5`
using `br ready --json --label first-playable` and
`bv --robot-next --label first-playable` until normal startup and continuous
Levels 1–4 work. Other workers may take independent ready families in parallel.
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

Use the smallest check that exercises the changed behavior. During iteration,
check the affected package/target; after a coherent feature change, run relevant
existing focused tests and its real producer-to-consumer path. An exact test
filter still compiles the package's test binary, so batch related edits and reuse
warm artifacts. Do not run a workspace suite or rebuild Unity after every tweak.
Avoid duplicate Cargo jobs for the same workspace/profile; coordinate shared
integration checks once against the integrated code and reuse their evidence.

Examples from `research/private/remc2-sim-rs`:

```sh
cargo fmt -p sim-core -- --check
cargo check -p sim-core --lib --locked
cargo test -p sim-core --lib --locked <test-name> -- --exact
```

Substitute the actual owner package and target (`--bin sim-cli` for the CLI).
Confirm the selected test ran; a generic scheduler test does not prove a new
creature or spell. Run scheduler/order checks when that shared path or its wiring
changes. Reuse a successful compile from the relevant test/build; repeat it only
after changes, failures or a new concern. Add a focused regression check for an
uncovered defect when useful; do not create a companion test project per bead.

Shared scheduler, arena lifetime, definitions, save/replay, network ordering and
ABI changes need their affected consumers and failure paths checked together.
At those integration points, run `cargo check --workspace --all-targets --locked`
once for the integrated revision when the blast radius warrants it, plus the
relevant integration tests. Full campaign/multiplayer assembly keeps its real
end-to-end acceptance. No arbitrary test-count gate or deferred bug cleanup.

For Rust changes exercised in Unity, run `cargo build -p sim-capi --locked` and
install the matching native library before play, even when the ABI is unchanged.
Rebuild the Unity player when C#, scenes, packaged assets or the native contract
require it; a
Rust-only change does not itself require a Unity rebuild. The current launcher
only starts the installed player; `mc-zdo.1` owns the reliable build/install and
native-only refresh command. Do not claim a fresh runtime result using a stale
plugin. Content-only edits use the affected importer/loader and actual runtime
path, without an unrelated Rust build.

Preserve the workspace's documented compiler stability settings and lockfile.
Keep target caches; do not clean or change profiles/flags to force a rebuild.
On `3090`, `/tmp` is RAM-backed tmpfs. Keep large Cargo caches, player copies
and proof outputs on disk, not `/tmp`. The current warm Cargo target resolves
to `/home/oliver/Projects/mc2-tmp26/cargo-cache-20261111` on SSD. Before heavy
work inspect available RAM, swap and memory pressure as well as disk headroom;
pause local heavy jobs under pressure. Free tmpfs space is not free disk.
Use configured RCH for expensive builds when its worker is healthy; retain the
local warm path for quick checks. Run RCH from the repository root: build scripts
read upstream remc2 files outside the Rust workspace. Pin the nested toolchain
explicitly; for example, after `rch check` confirms the worker is available:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo +1.98.0 check --manifest-path research/private/remc2-sim-rs/Cargo.toml -p sim-input --locked
```

Select the actual affected package/target. `.rchignore` retains source tables,
compile-time regression fixtures and atlas CSVs but omits large owner runtime
caches and Unity outputs. Run checks needing those omitted assets locally.
RCH 2.1.34's optional `--source-content-receipt` rejects this repository's source
symlinks; omit that option. Require the remote worker/result in RCH output and
preserve the source closure. Offload success is not a gameplay or speedup claim.
Run changed-source UBS at handoff, with its scope limits. Documentation/Bead-only
changes need diff/graph checks, not Cargo.

Launch the existing local player from the repository root:

```sh
research/private/remc2-unity-native/run-campaign.sh
```

The launcher needs the built Unity player, matching native plugin and ignored
owner packages/caches. Its current entry support is defined by the code.
