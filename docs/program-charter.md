# Carpet Flight program charter

Status: **binding operator direction, accepted 2026-08-21**
Repository: `/home/oliver/Projects/odcpw/magiccarpet`
Scope: private engine development and owner-local MC2 compatibility; commercial
release remains a later explicit gate.

## Purpose

Build one extensible game engine, **Carpet Flight**, from the working remc2 and
Rust/Unity implementation already in this repository.

The first canonical game configuration is privately playable Magic Carpet 2
using Oliver's owned local assets. The same engine must then support replacement
assets, reskinning, retuning, changed flight and spell rules, changed enemy AI,
new worlds, and eventually a substantially different independently branded
game. The purpose of the MC2 path is both preservation and a demanding real
first customer of the engine.

This direction supersedes the former two-product, separate-repository,
clean-slate C# successor, browser-first, and level-by-level-before-API plans.
Those records may retain useful evidence, but they no longer select work.

## Permanent architecture decisions

1. **One engine.** There is no second successor gameplay implementation and no
   presumed sibling `carpet-flight` repository.
2. **remc2 is the canonical MC2 source implementation.** Its behavior and the
   accepted Rust port provide the compatibility reference for `MC2-owned`.
3. **Rust owns gameplay authority.** `sim-core` or a focused Rust domain crate
   owns definitions, sessions, ordering, RNG, rules, AI, world state, hashes,
   saves and replays. No client owns a parallel simulation.
4. **Unity is the primary player and editor.** It owns input adaptation,
   presentation, audio playback, HUD, tools and the future OpenXR/VR shell.
   Unity physics is not authoritative gameplay physics.
5. **Three.js is an inspector and optional client.** It provides rapid
   visualization, authoring, replay inspection and browser experiments over the
   same Rust contract. It is not a competitor to Unity or a release gate.
6. **The AI harness uses a semantic API.** It discovers and changes supported
   game meaning through typed, versioned operations. It never edits raw memory,
   guesses source offsets or mutates a client behind the engine's back.
7. **Games are packages over capabilities.** Packages provide definitions,
   bounded behavior graphs, worlds, presentation bindings and assets. The Rust
   engine alone provides compiled capabilities.

## Canonical product sequence

### Foundation proof

Build the authoring boundary around the successful Level 1 route before broad
campaign expansion:

1. extract a transport-neutral Rust `GameAdapter`/`GameSession` contract while
   keeping the sealed Level 1 implementation intact;
2. define the versioned package, immutable `GameDefinition`, semantic catalog,
   definition transaction and client contracts;
3. expose `MC2-owned/level1` through those contracts and preserve a fixed,
   non-no-op accepted compatibility corpus through a built native consumer;
4. prove Unity plus a minimal headless TypeScript consumer observe the same
   normalized Rust domain values and cannot alter gameplay by polling; and
5. run a synthetic `custom-proof` package that changes one asset, one flight
   parameter, one spell rule and one enemy policy with byte-identical engine
   and client executables.

Only package artifacts may differ between baseline and custom runs. Each of the
four changes needs a causal one-factor observation and a negative control.

### MC2 completion

After the foundation proof, migrate the remaining MC2 campaign into one shared
engine. Later levels populate package data or add reusable compiled
capabilities; they do not create Level-N public sessions, ABIs or Unity
authorities. Existing Level 2 work remains valuable implementation evidence but
is downstream of the neutral foundation.

### Original Carpet Flight product

Use the same engine and semantic API to replace MC2-specific content and evolve
the game into an independent commercial successor. Desired later capabilities
include original art and audio, improved flight/combat/AI, native VR, generated
asset workflows, real-city world packs, voxel battlefields, destruction and
water simulation. Each enters as a named package, provider, compiled capability
or client adapter; none is permission to fork the engine.

## Semantic authoring boundary

The stable operation family is:

```text
capabilities  describe  get  set(stage)  validate  diff
apply  apply-status  rollback  reset  snapshot
```

`set` stages only. `validate` and `diff` do not mutate. `apply` compare-and-swaps
one immutable definition revision into an authoring workspace and returns a
durable receipt. Existing sessions remain on their old definition until an
explicit reset or reopen binds the new one and advances session generation.
Rollback is a new guarded forward transaction to a retained content-addressed
graph, not an unconditional history rewind.

Every writable descriptor names a stable semantic ID, type, units, bounds,
default/effective value, dependencies, side effects, provenance and mutation
timing. Scheduled compiled capabilities also name their producer, dispatch
phase/key, handler, same-pass epilogue, following-pass cleanup, authoritative
read/write set and RNG/event effects.

## Authority and determinism invariants

- Rust is the sole writer of live simulation state.
- A validated immutable definition and content-addressed package/artifact graph
  are the only package inputs to a session.
- Client observation is read-only. Zero, one or many presentation polls between
  identical steps must not change simulation hashes, RNG or gameplay events.
- Preserve fixed-width/wrapping arithmetic, relevant source order, RNG draw
  order, event/audio order, reset isolation, save/replay identities and hashes.
- Source class/model/action/slot identifiers remain inside the MC2 compatibility
  adapter; public authoring uses semantic IDs.
- Packages contain no arbitrary native code, generated Rust or hidden I/O.
- Unknown schema, capability, dependency, reference, hash, provenance or timing
  fails closed before publication.

## Private data and later release

This repository is a private lab. Owned MC2 data, derived caches, captures and
source-shaped compatibility artifacts remain owner-local and outside releases.
They are valid private runtime inputs and do not block engine work.

A commercial/public build is a later, explicit review of content rights,
provenance, dependencies, telemetry, privacy, naming and distribution. That
review may require original replacement content; it does not force a separate
engine or a fictitious internal clean-room transfer process today.

## Work-selection law

- The canonical recovery and execution plan is
  [`carpet-flight-engine-plan.md`](carpet-flight-engine-plan.md).
- Until its two-package foundation graph is accepted, do not activate broad
  Level-N, monolith-rewrite, browser, multiplayer, voxel, city, VR or release
  waves.
- A feature is not delivered by a schema, helper, mock, direct setter, internal
  test or refusal path. Its positive path must cross the nearest real consumer.
- Decomposition follows a consumer-owned semantic seam. Moving code without
  establishing that seam is not progress.
- Existing Beads and historical plans are evidence indexes, not authority.
  Reconcile them to this charter before selecting work.

## Canonical documents

Read these in order:

1. this charter;
2. [`product-promise.md`](product-promise.md);
3. [`modular-engine-game-module-contract.md`](modular-engine-game-module-contract.md);
4. [`stack-decision.md`](stack-decision.md);
5. [`private-public-boundary.md`](private-public-boundary.md) and
   [`provenance-policy.md`](provenance-policy.md);
6. the temporary recovery and execution plan.

`AGENTS.md` and `README.md` summarize current repository operation. Other design
documents are subordinate capability notes or dated historical evidence.

## Change control

Only a new explicit operator decision may reopen one engine versus two, Rust
authority, Unity's primary role, Three.js's inspector role, or `MC2-owned` as
the first package. New evidence may refine package schemas, transports,
conformance cases and implementation order without silently changing those
decisions.
