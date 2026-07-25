# Original successor engine and game-module contract

Version: 1.0  
Status: successor-track architecture baseline; exact stack pending proof  
Track: `S`

## Decision

Build a reusable deterministic carpet-flight engine for the original
successor. MC2 is not a module, dependency, compatibility target, or privileged
mode of this engine.

The first production module is the successor's original island/city game.
A tiny synthetic module proves that engine capabilities are not hard-coded to
that product.

## Architectural laws

1. One authoritative simulation truth across Chrome, native, server, replay
   and tests.
2. No MC2 reconstructed code, state shape, catalog, name, realm, original
   asset, owner-data adapter or compatibility behavior in the successor engine.
3. Capabilities, not product-name switches.
4. Presentation and accessibility cannot mutate authority.
5. Browser-delivered mods are validated data/assets/bounded graphs, not
   arbitrary C#, native libraries or downloaded Wasm.
6. Exact content/rules/build identity before room admission or save/replay use.
7. Deterministic ordering, RNG, serialization, hashing and first-divergence
   diagnostics.
8. Explicit schema/version migration; no guessed compatibility.
9. Safe package absence/removal and atomic activation.
10. Every asset and package has provenance, license and budget metadata.

## Layers

```text
Successor.Contracts
  <- Successor.Determinism
  <- Successor.Simulation
  <- Successor.Module.Runtime
  <- Successor.Persistence
  <- Successor.Network.Protocol

Successor.Module.Runtime
  <- Successor.Client
  <- Successor.Server
  <- Successor.Replay
  <- Successor.Tools
```

Names are logical. Unity assembly names are chosen only after the stack gate.

## Generic capability families

- carpet-like kinematic flight;
- spatial and visibility queries;
- world surfaces, water, hazards and deterministic edits;
- bounded structural cells/destruction;
- health, damage, effects and relationships;
- finite-resource custody and transport;
- stronghold/reserve/checkpoint roles;
- commands, projectiles and abilities;
- objectives, scheduled events and results;
- AI perception, policy and command interfaces;
- information eligibility, HUD/map read models and presentation cues;
- save/checkpoint/replay/migration; and
- room admission, snapshots, repair, reconnect and results.

A capability enters the engine only when both the original production module
and synthetic module can use it without successor-specific names or rules.

## Package kinds

| Kind | Owns |
|---|---|
| `GameModule` | Root original game identity, capabilities, schemas, defaults |
| `BehaviorLibrary` | Bounded effects, objectives and AI policy nodes |
| `WorldPack` | World topology, spawns, encounters, objectives and budgets |
| `RulesProfile` | Factions, costs, damage, death, objective and victory laws |
| `PresentationPack` | Original/licensed models, materials, VFX, audio, UI |
| `LocalizationPack` | Strings, captions, narration and layout metadata |

There is no `PrivateOwnedDataOverlay` in Track S.

## Deterministic behavior

Validated graphs may describe effects, objectives, AI selectors, ecology,
progression, UI information and presentation cues. They have:

- typed units and stable IDs;
- deterministic iteration/RNG;
- explicit state reads/writes and information eligibility;
- bounded iterations, fanout, allocations, events and cost;
- declared authoritative versus presentation domain;
- schema owner and failure behavior.

Reject unbounded loops/recursion, dynamic code, reflection, ambient clocks/RNG,
filesystem/network access, hidden global state, target-specific authoritative
behavior and presentation-to-authority callbacks.

## Session identity

```text
EngineApiVersion
+ SimulationBuildHash
+ ProtocolVersion
+ GameModuleSetId
+ ContentRoot
+ RulesProfileId
+ RulesHash
```

Track P owner-data identity is never part of a Track S session.

## Saves and replays

Record the compatibility tuple, package graph, schema versions, module state,
world edit tail, canonical checkpoint hash, command stream and migration
history. Outcomes are exact open, named deterministic migration, safe blocked
dependency state, or corrupt/unsupported rejection.

## Synthetic conformance module

Before the social demo depends on the module system, prove:

- engine boot with the production module absent;
- one controllable mover, resource, effect, objective, AI policy, result,
  world, cue, save and replay;
- Chrome/native/server identical roots and replay hashes;
- install/upgrade/migration/rollback/remove/reinstall;
- teardown without leaked IDs/state;
- malformed/oversized/revoked/corrupt package rejection; and
- static dependency scan finding no MC2/reconstruction reference.

## Explicit non-goals

- MC2 campaign/catalog compatibility;
- executing/importing owner MC2 data;
- sharing packages with Track P;
- browser arbitrary-code mods;
- hot-swapping authority during a match;
- peer authority; and
- claiming Unity, C#, Rust or another stack before its gate passes.

