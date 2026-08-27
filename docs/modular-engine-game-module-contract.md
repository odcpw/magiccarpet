# Carpet Flight engine, package and semantic API contract

Status: **governing architecture contract; planned surfaces are not yet
implemented unless the evidence-first plan says otherwise**
Authority: [`program-charter.md`](program-charter.md)

The concrete definition composition, strict agent-authoring boundary and
first-three-level runtime milestone are binding in
[`agent-addressable-game-architecture.md`](agent-addressable-game-architecture.md).

## Decision

Carpet Flight is one Rust-authoritative engine with versioned game packages and
multiple non-authoritative clients. `MC2-owned` is the first compatibility
package. A synthetic `custom-proof` package is the first independence witness.
Unity is primary; Three.js is an inspector and optional client.

Current `Level1*`/`Level2*` sessions, source-shaped packages and Unity wrappers
are retained behind compatibility adapters. They are implementation inputs,
not the permanent public language.

## Vocabulary

- **Engine:** Rust domain code that validates definitions, owns sessions,
  executes compiled capabilities and emits authoritative observations.
- **Compiled capability:** a versioned Rust algorithm. Only an engine build
  provides one.
- **Game package:** a content-addressed graph of definitions, rules, bounded
  behavior graphs, worlds, presentation bindings and assets. It may require
  compiled capabilities but never supply native code.
- **Game definition:** one immutable, fully validated semantic model assembled
  from a package graph.
- **Compatibility adapter:** a bounded translation between source-shaped data
  or behavior and the neutral domain. remc2 IDs remain private to this layer.
- **Workspace:** the authoring identity whose active-definition pointer is
  changed by guarded publication.
- **Session:** a Rust-owned live simulation bound to one immutable definition.
- **Client:** a command producer and observation consumer with no gameplay
  writer. Unity and Three.js are clients.
- **Asset source/runtime asset:** provenance-bearing input and its validated,
  content-addressed runtime result.

## Layering

```text
asset sources/importers
        -> validated package + artifact graph
        -> immutable GameDefinition revision
        -> Rust GameAdapter/GameSession authority
        -> canonical commands/snapshots/events/audio/presentation bindings
        -> C ABI -> Unity
        -> process/Wasm/HTTP -> headless TypeScript / Three.js

AI harness / human tools
        -> semantic describe/get/set/validate/diff/apply/rollback
        -> workspace definition publication
        -> explicit session reset/reopen
```

The canonical domain interfaces live in `sim-core` or a focused Rust domain
crate. `sim-capi`, CLI, MCP and HTTP/Wasm are transports. A transport may encode
the contract differently but cannot add meaning, validation bypasses or
gameplay state.

## Package manifest

Every accepted package graph names:

- schema version, package ID/version and compatible engine-contract range;
- canonical package graph hash, dependencies and deterministic load order;
- exact compiled capability requirements and optional-requirement behavior;
- rules profiles, behavior graphs, worlds, presentation and asset bindings;
- stable semantic IDs and compatibility mappings owned by adapters;
- source, recipe, runtime and distribution provenance;
- size/count/graph/resource limits;
- save/replay compatibility and explicit migration declarations; and
- all source/runtime artifact hashes.

The engine provides compiled capabilities. Packages provide only data,
declarative bounded graphs, assets and bindings. Unknown members or required
capabilities, dependency cycles, semantic-ID collisions, path escapes, hash
mismatches, invalid provenance or unsupported migrations fail before
publication or session creation. Optional behavior never silently falls back to
another package or Level 1.

## Capability and descriptor identity

A compiled capability has a stable semantic ID and version, typed inputs and
outputs, deterministic constraints and availability. If it participates in a
scheduled behavior lifecycle it also declares:

- dispatch phase and key;
- ordinary producer/constructor;
- handler;
- same-pass epilogue/hide behavior;
- following-pass cleanup/reclaim;
- authoritative read/write set; and
- RNG, event and ordering effects.

An authorable descriptor names:

```text
id, descriptor_version, kind, value_type, unit,
default_value, bounds_or_enum, access, mutation_timing,
determinism_role, dependencies, side_effects, scope,
provenance, deprecation, summary, examples
```

Stable IDs describe game meaning, for example
`flight.player.max_forward_speed`, not a Rust field, source address or remc2
slot. A field is not writable merely because it exists internally.

The initial mutation classes are tuning, content/assets, bounded declarative
behavior and compiled capability selection/configuration. New algorithms require
an engine build and cannot be uploaded through `set`.

## Semantic operation contract

```text
capabilities
describe
get
set             # stage and type-check only
validate        # full dry run; no mutation
diff            # typed semantic/graph diff; no mutation
apply           # guarded atomic publication
apply-status    # recover transaction disposition/receipt
rollback        # new guarded forward publication to retained graph
reset           # reset/rebind a session; does not edit a definition
snapshot
```

`open`, `step`, event paging and `close` are adjacent runtime operations. All
operations use stable versioned envelopes, typed errors, deterministic data
bytes, bounded pagination and data-only machine output. Optional CLI/MCP/HTTP
deadlines are transport policy, never a simulation outcome.

## Definition transactions

A change set contains a schema version, target workspace,
`expected_definition_hash`, `expected_definition_revision`, idempotency key,
typed replacements and requested timing. It does not target session generation.

Validation creates a candidate definition and complete bounded diagnostics but
does not publish. Apply compare-and-swaps the workspace's active-definition
pointer and atomically makes the definition, package/artifact graph, bindings
and transaction receipt visible. Authoritative roots are:

- workspace active-definition pointer;
- immutable definition and package graph;
- artifact store and active asset bindings;
- sessions and their definition bindings;
- simulation state, event/audio journals and cursors;
- Rust-owned presentation lifecycle/sidecars; and
- transaction receipt journal.

Failure before commit leaves every root unchanged. Cancellation after commit
is resolved by `apply-status`. Existing sessions remain bound to their old
definition until reset/reopen explicitly selects the new one and advances that
session's generation.

Rollback validates a retained prior graph and publishes it as a new definition
revision using CAS against the current definition. It does not rewind history,
guess “previous,” or silently rebind sessions.

## Session and client contract

Runtime identity includes:

```text
engine_contract
package_graph_hash
definition_hash + definition_revision
session_id + generation + tick
```

The Rust domain exposes generation-bound typed commands, idempotent complete
snapshots for named projections, ordered cursor-paged events/effects, reset and
replay receipts, and typed version/capability negotiation.

Presentation polling is observational. Rust schedules any source-required
notification or presentation feedback independently of client poll frequency.
Identical input traces with zero, one and many presentation reads between steps
must produce identical simulation hashes, RNG and ordered gameplay events.

Client parity compares canonical normalized domain values and canonical
digests, not C struct bytes against JSON. Each adapter round-trips fixed vectors
and rejects unknown gameplay fields. Rendering pixels may differ.

## Asset contract

An asset slot specifies kind, runtime format, dimensions, frames, palette/color
space, animation timing, coordinate convention, dependencies and performance
bounds. Publication records source hash, tool/model provenance, deterministic
recipe where applicable, runtime hash and binding hash.

Generative output becomes a hashed source artifact. The external model call
need not reproduce identical bytes; deterministic validation and transformation
start at the captured artifact. Catalog read-back alone is insufficient: the
new runtime hash must reach the actual Unity/Three.js binding and cache
invalidation boundary.

## Compatibility adapters

`MC2-owned` may use exact exporter records, owner cache identities, source
class/model/action/slot values and accepted behavior internally. Its external
surface is the same semantic definition/session/client contract used by
`custom-proof`.

Existing public Level 1/2 symbols are grandfathered only in an explicit
compatibility baseline manifest with owner and retirement disposition. New
package-specific symbols are forbidden outside compatibility namespaces.
Neutral engine modules may not branch on Level-N identity; MC2 level names are
allowed as package content.

## Foundation conformance

The binding executable gates are specified atomically in
[`carpet-flight-engine-plan.md`](carpet-flight-engine-plan.md):

- a fixed, versioned, non-no-op Level 1 corpus through a built native consumer;
- same-build comparison with the sealed legacy route without permitting
  co-regression;
- type-enforced descriptor bindings plus an independent required-surface census;
- transaction CAS, failure atomicity, two-session isolation and forward
  rollback;
- client normalized-value parity and polling invariance;
- typed asset slot provenance and real runtime binding;
- a structural baseline/guard for native, Rust, managed and TypeScript exports;
  and
- baseline, four one-factor variants and combined custom package using
  byte-identical engine and client builds.

Passing the foundation proves these bounded extension surfaces. It does not
prove complete MC2, arbitrary games, production generated art, VR, voxel
destruction, cities, water, multiplayer or public release.

## Extension law

- Later MC2 levels add package content or reusable compiled capabilities.
- Voxel/destruction/water are compiled Rust world/physics capabilities over a
  provider-neutral world contract.
- Real cities are provenance-bound streamed world packs.
- VR is a Unity/OpenXR input/presentation adapter over the same commands and
  observations.
- Three.js remains an inspector/optional client.

No extension may introduce a client gameplay writer, Level-N public authority,
arbitrary package code, raw-memory authoring, or a competing engine.
