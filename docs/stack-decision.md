# Carpet Flight stack decision

Status: **accepted foundation stack, 2026-08-21**
Authority: [`program-charter.md`](program-charter.md)

## Decision

Use the working Rust/Unity path as the engine foundation:

| Responsibility | Selected stack | Authority |
|---|---|---|
| Simulation, definitions, rules, AI, RNG, world state, saves/replays | Rust (`sim-core` or focused Rust domain crates) | Sole gameplay authority |
| Native boundary | Versioned Rust C ABI (`sim-capi`) | Transport only |
| Primary desktop player/editor and future VR | Unity `6000.3.20f1`, C#, URP/OpenXR when activated | Input/presentation/tooling only |
| Rapid inspector, authoring UI and optional browser client | TypeScript + Three.js over Wasm or process/HTTP | Non-authoritative client |
| First headless AI harness | Rust CLI with JSON/NDJSON | Semantic API transport |
| Optional agent bridge | MCP tools over the same Rust domain operations | Transport only |
| Source asset authoring | Blender and replaceable image/model/audio tools | Produce source artifacts; no runtime authority |
| Owner-data import | pinned remc2 exporter plus existing closure/cache tools | `MC2-owned` compatibility input |

## Why Unity remains primary

The repository already has a real Rust-to-C-ABI-to-Unity desktop route. Unity
provides the strongest practical path to authored presentation, editor
workflows, mature input/audio, profiling and native OpenXR VR. Replacing it now
would discard working capability without addressing the missing semantic API.

Unity must not absorb gameplay logic. C# validates and copies ABI/domain data,
maps input, presents observations and runs editor tools. Unity physics,
animation callbacks, frame rate and polling frequency cannot determine
authoritative outcomes.

## Why Three.js remains useful

Three.js gives a fast, inspectable surface for catalog browsing, semantic diffs,
asset previews, replay visualization and optional web experiments. It is also a
second consumer that can expose accidental Unity-only semantics.

The first gate needs only a minimal headless TypeScript reference consumer for
canonical command/snapshot/event vectors and polling invariance. A polished
Three.js renderer is later; it must not block the engine foundation.

## Why Rust remains authoritative

Substantial deterministic gameplay, source-order behavior, fixed-width
arithmetic, hashing, package loading and C ABI work already exist in Rust.
Keeping authority there preserves the strongest working component and supports
native, Unity, CLI, Wasm and service transports without multiple simulations.

The canonical neutral session and definition contracts belong in `sim-core` or
a focused Rust domain crate. The existing `InternalCampaignAdapter` in
`sim-capi` is useful lifecycle evidence but is ABI-shaped and cannot be the
permanent domain seam.

## Foundation toolchain pins

- remc2 source: submodule commit
  `6806c7204d17220ecc0430759f784aea8ee2ddb2`;
- Unity: `6000.3.20f1` (`c9ba695d4f07`);
- Rust manifests and lockfile in
  `research/private/remc2-sim-rs` are authoritative for exact compiler/crate
  resolution;
- owner packages and generated caches remain external runtime inputs; and
- editor/MCP/AI plugins are optional authoring aids, never build or player
  dependencies unless separately accepted.

Verify current third-party versions from primary vendor documentation before
implementation; this document fixes responsibilities, not all future package
versions.

## Transport boundaries

The Rust domain defines meaning. C ABI, CLI, MCP and Wasm/process/HTTP encode
the same operations and identities. Cross-client conformance compares canonical
normalized values and digests, not transport bytes.

The initial semantic operation family is `capabilities`, `describe`, `get`,
`set` (stage), `validate`, `diff`, `apply`, `apply-status`, `rollback`, `reset`
and `snapshot`, plus adjacent runtime `open`/`step`/events/`close`.

## Platform sequence

1. Canonical Linux development and headless Rust/CLI conformance on `3090`.
2. Unity Linux desktop integration against the same built plugin and owner
   inputs.
3. Minimal headless TypeScript contract consumer; later Three.js inspector.
4. Other desktop builds when the shared contract is stable.
5. Unity/OpenXR VR after the two-package proof and an explicit VR milestone.
6. Browser delivery only when it serves a named product goal and meets measured
   performance/feature constraints.

## Deferred technical choices

The foundation does not yet choose a voxel engine, water solver, city-data
provider, navigation stack, multiplayer model, generative model, asset
interchange winner or hosted service. Each future capability must evaluate the
current FrankenSuite option when relevant, state its deterministic/authority
boundary, and pass a real integration gate before adoption.

## Rejected foundation directions

- clean-slate C# gameplay simulation;
- separate successor engine/repository;
- Unity-versus-browser architecture bakeoff;
- browser-first or multiplayer-first release;
- completing broad Level-N ports before the semantic/package boundary;
- raw-memory or editor-field mutation as the AI API; and
- a new renderer used to conceal the absence of an authoritative domain
  contract.

These choices may be revisited only by a new explicit operator decision or
decisive runtime evidence, not by preference drift.
