# Carpet Flight / MagicCarpet private engine lab

This private repository develops one remc2-derived, Rust-authoritative game
engine with Unity as its primary client. Its first canonical package is
owner-local `MC2-owned`; the same engine is intended to support custom assets,
rules, flight, spells, enemy AI and worlds through a typed semantic API.

The repository is not a shipped product. During the 2026-08-21 recovery, no
GitHub releases were present. Local ignored Unity/player/plugin artifacts exist,
but they were not freshly certified by that planning task.

## Direction

- remc2 plus the successful Rust port are the MC2 compatibility reference;
- Rust owns definitions, gameplay, ordering, RNG, hashes, saves and replay;
- Unity owns input and presentation and is the future OpenXR/VR shell;
- Three.js is a rapid inspector/harness and optional client, not another
  simulation;
- the AI harness uses typed `describe/get/set/validate/diff/apply/rollback`
  semantics rather than raw memory or Unity field edits; and
- the next foundation is API-first around the working Level 1 route, followed
  by an `MC2-owned` plus `custom-proof` two-package gate.

See the binding [program charter](docs/program-charter.md) and the temporary
[evidence-first recovery and execution plan](docs/carpet-flight-engine-plan.md).

## Current production route

```text
pinned remc2 + owner data
  -> remc2-level-exporter
  -> owner asset closure + decoded cache
  -> Rust sim-core/session state
  -> versioned sim-capi native ABI
  -> Unity desktop presentation
```

This route contains substantial working Level 1 behavior and incomplete Level
2 work. It does not yet contain the neutral package definition, complete
semantic authoring transaction, custom package proof, current VR runtime,
voxel/city/water capabilities or a released commercial game.

## Repository map

- `upstream/magic-carpet-2-hd/` — pinned remc2 source implementation; normally
  read-only.
- `research/private/remc2-level-exporter/` — pointer-free owner-level package
  exporter.
- `research/private/remc2-level-asset-closure/` — reachable owner-asset
  closure.
- `research/private/remc2-unity-asset-cache/` — deterministic decoded runtime
  cache.
- `research/private/remc2-sim-rs/` — Rust simulation, C ABI, CLI and parked
  Wasm proof.
- `research/private/remc2-unity-native/` — Unity `6000.3.20f1` client/editor.
- `docs/` — current contracts plus subordinate/historical research.

Owner packages, caches, captures and generated artifacts are runtime inputs and
remain outside Git and distributable builds.

## Canonical documents

1. [Program charter](docs/program-charter.md)
2. [Product promise](docs/product-promise.md)
3. [Engine/package/API contract](docs/modular-engine-game-module-contract.md)
4. [Stack decision](docs/stack-decision.md)
5. [Private/public boundary](docs/private-public-boundary.md) and
   [provenance policy](docs/provenance-policy.md)
6. [MC2 compatibility guide](docs/preservation-track.md)

## Verification

Exact focused and integration commands live in [AGENTS.md](AGENTS.md) and the
component READMEs. Run the smallest route that reaches a change; a filtered
zero-test result, internal helper, fixture-only path or historical artifact is
not positive runtime evidence.

The canonical checkout is `/home/oliver/Projects/odcpw/magiccarpet` on `3090`,
branch `main`, private remote `odcpw/magiccarpet`.
