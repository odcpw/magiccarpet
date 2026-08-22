# Carpet Flight product promise

Status: **governing product intent under the 2026-08-21 charter**

## The promise

Carpet Flight will be a living carpet-flight engine and game platform whose
first complete configuration can play MC2 privately from owned assets, and
whose rules, worlds, actors, AI and presentation can then be changed through a
tight semantic API instead of another source-porting campaign.

The target experience preserves the qualities that make MC2 compelling:
immediate free flight, expressive terrain-scale movement, spell combat,
creatures and opponents that create a living battlefield, progression and
economy, readable objectives, dramatic audiovisual feedback, and deterministic
world consequences. It is not a museum wrapper. It is the foundation for an
independently branded, commercially viable successor with its own content and
new capabilities.

## Two user experiences, one product

### Player experience

- Launch a real game quickly and fly without fighting tooling.
- Feel speed, altitude, banking, hover, momentum and terrain proximity clearly.
- Cast understandable, consequential spells in reactive battles.
- Meet enemies and creatures whose choices are deterministic, inspectable and
  meaningfully different.
- Play a complete campaign through one engine rather than disconnected level
  demos.
- Later choose desktop, conventional controller, or native Unity/OpenXR VR
  without changing simulation authority.

### Creator and AI-harness experience

- Ask the engine what can be changed and receive bounded typed descriptors.
- Change assets, flight tuning, spells, actors, enemy policies, objectives and
  worlds without locating Rust constants or Unity fields.
- Preview validation and semantic diffs before publication.
- Apply an atomic definition transaction, receive exact read-back evidence, and
  reset/reopen a session onto the new definition.
- Revert by publishing a retained prior definition under an optimistic guard.
- Use the same operations from a local CLI, Unity editor tooling, an optional
  MCP bridge and a Three.js inspector.
- Swap an expensive model for a cheap or local model without changing the
  authoring contract. Model output becomes a provenance-bearing source artifact
  and must pass deterministic validation/transformation from that boundary.

## First believable product proof

The foundation milestone is deliberately smaller than “port the campaign” and
stronger than “build an API scaffold.” It is complete only when:

1. `MC2-owned/level1` runs through a neutral Rust domain session and built
   native consumer against a fixed non-no-op compatibility corpus;
2. Unity and a minimal headless TypeScript consumer round-trip the same
   canonical domain observations, and polling either client cannot alter
   gameplay;
3. the authoring surface discovers the four first semantic targets with type,
   units, bounds, provenance, dependencies and timing;
4. baseline, four one-factor variants and one combined `custom-proof` package
   change an asset, flight value, spell rule and ordinary enemy policy path;
5. engine/plugin and client executable hashes stay identical across those
   runs, with only declared package artifacts differing; and
6. validation, CAS publication, failure atomicity, explicit session rebinding
   and forward rollback produce machine-readable receipts.

This proves a bounded but real engine boundary. It does not prove arbitrary
games, complete MC2 fidelity, production asset quality, VR, voxels, cities,
water, multiplayer or commercial release.

## Product horizons

### Horizon 1 — engine foundation

Protect the working Level 1 behavior while establishing neutral definitions,
packages, semantic authoring, transactionality, client observations and the
two-package conformance proof.

### Horizon 2 — complete MC2-owned

Bring the remaining campaign, rules, enemy lifecycles, economy, spells,
objectives and presentation through the same contracts. A level is content plus
reusable capabilities, never a new public engine surface.

### Horizon 3 — original Carpet Flight

Replace the private MC2 content with an original art direction, worlds,
characters, audio and identity; retune and extend gameplay through the semantic
surface; then make a deliberate commercial release decision.

### Horizon 4 — advanced worlds and embodiment

Add Unity/OpenXR VR, provider-neutral real-city worlds, voxel battlefields,
destruction and water as tested client adapters, world providers and compiled
Rust capabilities. These are downstream ambitions, not foundation
dependencies.

## Quality floor

- Determinism, reset/replay and clear provenance are product features.
- Real runtime observations outrank screenshots, mocks and internal tests.
- Unity presentation should feel authored, not like a debug view.
- AI-created assets are not accepted merely because they render; they must meet
  typed slot, animation, scale, coordinate, performance and binding contracts.
- Errors must identify the semantic path, violated rule, current identity and a
  safe next operation.
- The engine must remain usable without a cloud model or a particular editor
  automation plugin.

## Explicit non-goals for the foundation

- no separate successor repository or clean-slate gameplay core;
- no browser-first or multiplayer-first launch gate;
- no full monolith rewrite before the contract owns the seam;
- no raw-memory or arbitrary string-key mutation API;
- no arbitrary executable code in packages;
- no broad Level 2/later-level expansion before the two-package proof;
- no claim that owner-local MC2 artifacts are commercially distributable; and
- no premature VR, city, destruction, water or generative-content production
  wave.

## Scope-change test

A proposed direction change must state which player or creator outcome improves,
which accepted invariant changes, what current evidence requires the change,
and what falsifier would reject it. “A different stack or architecture sounds
cleaner” is not sufficient to reopen settled decisions.
