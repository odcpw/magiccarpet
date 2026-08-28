# Preservation campaign seam geometry

> **Retained technical evidence, superseded work order as of 2026-08-21.** The
> measured Rust state/dispatch/fold findings remain useful, but Track P/Track S
> separation and Level-2-before-semantic-API ordering do not govern. Reuse this
> material only behind the one-engine contract and current recovery plan.

Version: 1.0
Status: historical campaign-fold research; implementation priority superseded
Evidence revision: `ad6458dde700451c466758ea5e4c1f489b9eff2c`
Pinned remc2 revision: `6806c7204d17220ecc0430759f784aea8ee2ddb2`
Track: `P` only

Research basis and falsifiable extensions are in
[`preservation-program-folding-research.md`](preservation-program-folding-research.md).

## Purpose

This memo defines the smallest source-faithful Rust architecture that should
let the private preservation engine finish Level 2 and then absorb the
remaining qualified owner campaign as mostly data plus genuinely new shared
behavior. The owner-campaign census, not this memo, determines the final realm.

Its consumers are milestone 0, the folded Level 2 closure, and later-realm
implementation. It exists because the current Level 2 implementation has
measured duplicate mutable authority, manual synchronization, and
level-number-specific routing. It should be superseded when the generalized
campaign architecture is implemented and its invariants live next to code and
tests.

This is private reconstruction knowledge. It does not amend
`modular-engine-game-module-contract.md`, which governs the independently
authored Track S successor. No source-derived type, table, state layout,
behavior, or artifact described here crosses into Track S.

## Decision in one sentence

Preserve remc2's deterministic schedule as the engine backbone; give every
mutable fact one canonical owner; express a level as immutable initial data and
source facts; and implement behavior once, keyed by the same source dispatch
site plus class/action topology that selects it in remc2.

The target is not the maximum number of modules. It is the minimum amount of
mutable information crossing module boundaries, subject to exact source
semantics.

## Scope and non-goals

This memo authorizes only the milestone-0 sealed Level 1 facade before
`mc-campaign-fold-contract-r78sq` is adopted. Remaining Level 2 work continues
transition-style under its existing Beads; the semantic fold and authority
changes remain gated. Narrow blocker-driven seams for exact Level 2 behavior,
notably simulation inputs currently misclassified as presentation assets, stay
within those Level 2 Beads.

The following are explicit non-goals:

- no runtime plug-in framework, ECS migration, behavior DSL, service graph, or
  dependency-injection container;
- no trait-object dispatch in the source-ordered hot path;
- no `Level3Session`, `Level4Session`, or one ABI entry point per level;
- no attempt to make modules equal in size;
- no re-opening of sealed Level 1 behavior merely to make it aesthetically
  match the new shape;
- no simultaneous physical file move and semantic state rewrite; and
- no architecture gate that delays a directly provable Level 2 capability.

## Contract standing and evidence notation

The sections headed **Contract v1** are normative for the proposed fold. `MUST`,
`MUST NOT`, and `MAY` have their usual requirements meaning. Later background,
formalization, and migration rationale explain the choice but do not broaden
it. If a conflict remains, the normative contract wins.

Every repository citation below was measured with `git show` at evidence
revision `ad6458dd`; every remc2 citation was measured at the pinned submodule
revision `6806c720`. Paths are repository-relative, and line numbers name those
exact revisions. A worker implementing after either file changes MUST remeasure
the cited range before relying on it.

This contract performs no implementation or ABI change and makes no Level 2
closure or Level 3 playability claim. It records—but does not originate—the
operator's sequencing decision on `mc-campaign-fold-contract-r78sq`: milestone
0 is exempt, transition-style Level 2 work continues, and `mc-fi4n` closes Level
2 on the folded engine. The parent may create the remaining proposed Beads only
after its own dependencies and operator gate are satisfied.

## Contract v1: decision ledger

| ID | Binding decision | Decisive measured evidence | Falsifier | Input status |
|---|---|---|---|---|
| D1 | Reuse the structurally generic campaign v2 selection, lifecycle, step snapshot, and world projections for descriptor-backed campaigns. Keep every family/Day query as a frozen Level 2 adapter. Add campaign-handle bridges for the generic effect journal and presentation frame; do not add a per-level ABI or a second selection layout. | `McCampaignSelectionV2` already carries external level, internal level, and map type (`research/private/remc2-sim-rs/crates/sim-capi/include/mc_sim_level2.h:19-28`), but the implementation still stores an explicit Level1/Level2 enum (`research/private/remc2-sim-rs/crates/sim-capi/src/native_v2.rs:7954-7965`), reset mutates only Level 2 (`native_v2.rs:8371-8388`), and step rejects every non-Level-2 campaign (`native_v2.rs:9237-9243`). | A validated remaining-realm package needs identity-critical open input that cannot be derived from the package plus the existing selection triple, or a real common consumer cannot be expressed by the retained projections plus journal/frame bridges. Then add one versioned generic field/layout, not a level API. | Decision complete; remaining-realm census `mc-iol7n` is missing, so the falsifier has not yet been exercised on Level 3+. |
| D2 | Introduce one hash-bound `campaign-definition.json` member inside a fail-closed campaign package. It contains a typed `CampaignSpec`, immutable `SourceFacts` index, and the exact executable reset-root requirements for the captured post-`GenerateEvents` state. Owner bytes stay in separately hashed members; presentation materialization cannot feed simulation. | The exporter already hashes every member and the ordered member identity (`research/private/remc2-level-exporter/src/exporter_main.cpp:1900-1930`) and emits level identity (`exporter_main.cpp:1946-1953`). Collision masks already demonstrate a simulation fact selected from authored rows, with typed bytes and source-file hashes (`exporter_main.cpp:411-425`, `433-525`). The Rust loader rejects unknown members, sizes, member hashes, and package-hash drift (`research/private/remc2-sim-rs/crates/sim-core/src/native_world.rs:27150-27242`). | A reached callback reads owner data that cannot be represented as a bounded immutable typed fact or reset state without evaluating arbitrary executable package content. The schema then blocks; it does not admit a DSL. | Envelope complete. `GOLDEN-CHANGE` approved 2026-08-23: dormant pre-generation authored rows are inventory, not executable requirements. Later runtime selectors remain compiled-registry authority and fail transactionally before callback/epilogue when unknown. |
| D3 | Use an explicit, compiled Rust registry as production authority, keyed by source dispatch site plus the selector shape used at that site. Use a deterministic read-only extractor only as a provenance/drift cross-check; it MUST NOT emit behavior code. | Pinned remc2 has a static 17-row class table (`upstream/magic-carpet-2-hd/remc2/engine/EventsFunctions.cpp:2060-2078`). `ApplyEvents_498A0` indexes it by class/action inside a rescan loop (`upstream/magic-carpet-2-hd/remc2/engine/Events.cpp:410-470`), while `UpdateEntities_57730` performs prepasses and independently indexes the same table (`EventsFunctions.cpp:40093-40172`). Current Rust instead constructs a Level-2-only route vector and numeric route tags (`research/private/remc2-sim-rs/crates/sim-core/src/native_world.rs:21728-21910`). | The registry probe demonstrates that an explicit registry cannot be kept complete by a deterministic cross-check, while a source-derived declaration generator can emit stable selector declarations without generating behavior or erasing dispatch sites. Only declaration generation may then replace hand-authored keys. | Architectural choice complete; empirical drift counts from `mc-campaign-registry-probe-udemg` are missing. |
| D4 | A `CampaignSession` owns immutable `Arc<CampaignDefinition>` and `Arc<ResetState>` plus one live `CampaignState`. `PersistentState`, `ResetState`, and `ScratchState` are disjoint. Transactional step clones only live state; reset restores the one baseline and invalidates scratch. | `Level2ResetSession` interleaves baseline/live twins across its outer fields (`native_world.rs:16692-16810`), copies terrain into three nested domains before dispatch (`native_world.rs:20078-20086`), and restores fields manually (`native_world.rs:24010-24063`). The ABI currently clones the entire reset session and commits only after bounded output succeeds (`native_v2.rs:9282-9339`). | The state census finds a source-observable value that cannot be assigned exactly once to immutable definition, persistent state, reset state, canonical topology, journal, or derived scratch; or replay requires two writable mirrors. Migration stops until the ownership model is refined. | Carrier and proof relation complete; the field-by-field classification in `mc-campaign-state-census-dxvvd` is missing. |
| D5 | Land milestone 0 as the sealed Level 1 facade, then land the semantic fold as the dependency graph below: evidence seal, pure moves, descriptor/registry, one carrier cut at a time, scheduler cutover, generic ABI, managed consumer, folded Level 2 close, Level 3+ saturation, and terminal Level 1 re-expression. No leaf combines a physical move with a semantic ownership change. | Current source pass mixes prepasses, route construction, dispatch, reconciliation, and player stepping in one method (`native_world.rs:21728-21977`); outer-turn ordering and reset are adjacent but distinct (`native_world.rs:23950-24063`). The C ABI/managed boundary is a separate transaction and compatibility surface (`mc_sim_level2.h:667-815`; `research/private/remc2-unity-native/Assets/Scripts/NativeCampaignSession.cs:95-176`). The sealed Level 1 implementation already has one owned `SessionRecord` with open/reset/step entry points (`native_v2.rs:2469-2508`, `8176-8208`, `10538-10548`, `10712-10726`). | A proposed leaf cannot be landed and reverted independently, needs two simultaneous writers on one logical carrier, or has no old/new observation test. Split or reorder that leaf before dispatch. | Proposal complete; milestone 0 is `mc-fold0-level1-facade-uf827`; the remaining graph is adopted by `mc-campaign-fold-contract-r78sq`. |

## Contract v1: complete Level 1 lifecycle

Level 1 has three deliberately different roles. It is an early facade proof, a
standing shared-substrate regression oracle, and the final descriptor migration
after the shared capability set is mature. It is not the oracle for Level 2
state surgery: sealed Level 1 does not execute `Level2EconomyState`,
`Level2ResetSession`, or the current Level 2 route table.

### Milestone 0: sealed internal facade

`mc-fold0-level1-facade-uf827` is the only implementation milestone exempt from
the fold-contract gate. It starts after this memo lands. It adds no public C
symbol, header entry, state carrier, descriptor, behavior registry, route, or
Level 2 change.

The concrete internal shape is static adapter dispatch, not a runtime plug-in
or trait object:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct InternalCampaignKey {
    external_level: u16,
    internal_level: u16,
    map_type: u8,
}

struct InternalCampaignOpen<'a> {
    package_dir: &'a Path,
    closure_path: Option<&'a Path>,
    owner_cache: Option<&'a Path>,
}

trait InternalCampaignAdapter {
    type Session;
    type StepInput;
    type Snapshot;

    const KEY: InternalCampaignKey;

    fn open(key: InternalCampaignKey, input: InternalCampaignOpen<'_>)
        -> AbiResult<Self::Session>;
    fn step(session: &mut Self::Session, input: Self::StepInput)
        -> AbiResult<Self::Snapshot>;
    fn reset(session: &mut Self::Session) -> AbiResult<Self::Snapshot>;
}

struct CampaignFacade<A>(PhantomData<A>);
struct SealedLevel1Adapter;

impl<A: InternalCampaignAdapter> CampaignFacade<A> {
    fn open(key: InternalCampaignKey, input: InternalCampaignOpen<'_>)
        -> AbiResult<A::Session> {
        A::open(key, input)
    }

    fn step(session: &mut A::Session, input: A::StepInput)
        -> AbiResult<A::Snapshot> {
        A::step(session, input)
    }

    fn reset(session: &mut A::Session) -> AbiResult<A::Snapshot> {
        A::reset(session)
    }
}
```

Exact local spelling MAY differ, but the following shape is binding:

- `SealedLevel1Adapter::KEY` is exactly external 1, internal 0, Night/map type
  1. Any other key fails before package load, registry insertion, or mutation.
- Its open input is the existing package path, closure path, and optional
  owner-cache root. `open` delegates to the same `NativeWorld::load` or
  `NativeWorld::load_with_owner_cache` and then the same `SessionRecord::new`.
  Those are the current sealed path (`native_v2.rs:8176-8208`; constructor at
  `native_v2.rs:3249-3373`).
- Its session type is the existing `SessionRecord`, not a copied
  `Level1Session` replacement. That record already owns the world, flight,
  terrain, mission, generation, journal/audio, and presentation state
  (`native_v2.rs:2469-2508`).
- `step` calls the existing `SessionRecord::step` exactly once and returns the
  existing snapshot; `reset` calls the existing `SessionRecord::reset` exactly
  once and returns the existing snapshot. The underlying implementations stay
  at `native_v2.rs:3376-3405` and `5368-5415` (continuing beyond both cited
  ranges).
- `mc_native_session_open`, `mc_native_session_step`, and
  `mc_native_session_reset` call the internal facade with the sealed key, but
  keep their signatures, registry, status mapping, validation order, handle
  identity, snapshots, hashes, generation, cursor, and failure behavior.
- Milestone 0 does not change the current `CampaignSessionKind::Level1` branch
  or make `mc_native_campaign_session_step` accept a Level 1 campaign handle;
  that would change an existing public negative. The facade sits beneath the
  sealed `mc_native_session_*` lifecycle until the gated generic-ABI cutover.
- The negative test calls the internal facade with an invalid key and proves
  no loader call, handle insertion, generation change, or journal mutation.
- No unused descriptor variant or placeholder adapter is added. The compiled
  descriptor adapter arrives only in the gated fold bead that has a real Level
  2 consumer.

The milestone-0 implementation belongs beside the private `SessionRecord` in
`sim-capi/src/native_v2.rs`; it does not move the sealed mission into a new
module. Tests may call the private generic directly to exercise the invalid-key
branch, while the exported functions prove the ordinary path.

This static generic is zero-cost and does not violate the no-trait-object hot
path rule. Its purpose is one selection/lifecycle seam with a real sealed
consumer, not premature polymorphism.

Milestone-0 acceptance is the complete existing Level 1 corpus plus replay and
reset hashes at the exact candidate revision, with byte-identical receipts; an
invalid selection negative; the focused Level 2 guards; format and focused
lint. “The adapter compiled” is not acceptance.

### Standing invariant during the fold

After milestone 0, every fold step MUST keep the sealed Level 1 corpus and
accepted hashes byte-identical. The author runs the focused affected Level 1
receipts; the integration owner runs the complete corpus once on each semantic
candidate revision. An unchanged docs/tracker-only revision does not replay the
corpus. A Level 1 failure blocks that candidate as a shared-substrate regression
but does not diagnose Level 2 carrier equivalence by itself.

The sealed adapter remains the Level 1 production path throughout Level 2 and
Level 3+ capability accumulation. No intermediate cleanup may rewrite its
mission, state, hash, corpus, public ABI, or qualification identities merely to
match the new architecture.

### Terminal milestone: descriptor re-expression

Only after shared behavior capability has saturated across folded Level 2 and
representative later realms may a terminal Bead re-express Level 1 as a
`campaign-definition.json` plus shared compiled behaviors. At that point the
schemas and handlers are constrained by richer levels, so Level 1 should add
only its immutable descriptor, mappings, SourceFacts, and any behavior proven
genuinely absent.

The terminal candidate runs old sealed and new descriptor paths as dual
observers over the frozen corpus. Acceptance requires byte-identical replay of
every frozen corpus observation—reset/state/arena/terrain/RNG hashes, ordered
effects/audio, objectives, and presentation receipts—and a fresh 182/182 matrix
rerun at the exact revision. No golden is regenerated and no tolerance is
widened. The sealed path retires only after both gates pass; before that match,
the descriptor path is candidate evidence, not authority.

## Contract v1: generic campaign ABI subset

The binary decision is: **do not introduce a v3 generic ABI**. The existing v2
selection and common lifecycle/projection layouts can select and carry Level
3+, so no existing layout or symbol-signature changes. The complete generic
subset cannot be reached with literally zero new exports because campaign
handles cannot use the current Level 1 registry's generic event pager or
presentation-frame copy.
Two additive campaign-handle bridges reuse those already-public v2 layouts.
That is smaller and more compatible than a parallel versioned lifecycle, while
still making the frozen Level 2 family surface an adapter rather than the
engine contract.

### Selection and authority

`McCampaignSelectionV2` is sufficient as the caller's identity assertion. Its
three semantic fields are `(external_level, internal_level, map_type)` and all
other bytes are reserved (`mc_sim_level2.h:19-28`). The package descriptor is
authoritative: open MUST compare the selection triple with the validated
descriptor and MUST reject a mismatch before allocating a handle. The package
hash and descriptor member hash bind the rest of the identity; a C caller does
not duplicate those hashes in a new selection struct.

The present hard-coded profile selector recognizes only Level 1 Night and Level
2 Day (`research/private/remc2-sim-rs/crates/sim-core/src/mission_content.rs:440-458`).
Replacing that branch with validated descriptor selection is an implementation
task, not evidence that the selection layout is deficient.

### Retained common surface

The following existing v2 layouts and symbols become the generic campaign
surface without a layout change:

| Responsibility | Retained contract |
|---|---|
| identity | `McCampaignSelectionV2` |
| one outer turn | `McCampaignStepInputV2`, `McCampaignStepSnapshotV2`, `mc_native_campaign_session_step` |
| lifecycle | `mc_native_campaign_session_open`, `mc_native_campaign_session_open_with_asset_cache`, `mc_native_campaign_session_destroy`, `mc_native_campaign_session_reset` |
| identity and common world projection | `mc_native_campaign_world_info` |
| terrain | `mc_native_campaign_terrain_state`, plane length/copy, and terrain-entity-index copy |
| common rows | player/objective/entity count and copy functions |

Those symbols already occupy the generic-named range
`mc_sim_level2.h:667-774`. Their semantics after fold are:

1. `open` validates the complete package, definition, SourceFacts, behavior
   closure, and reset construction before inserting a handle;
2. `reset` restores `ResetState`, preserves only classified
   `PersistentState`, increments generation, and invalidates old cursors;
3. `step` checks version, size, reserved bytes, generation, bounds, and command;
   clones only `CampaignState`; executes exactly one source outer turn; computes
   projections; and commits only after every requested bounded output succeeds;
4. every tick-sensitive copy is bound to generation and tick/state hash; and
5. unknown descriptor values, selectors, capabilities, objective operations,
   source facts, handles, generations, and cursors fail closed without mutation.

The existing implementation supplies the transaction precedent: it clones a
candidate at `native_v2.rs:9282`, derives the snapshot at
`native_v2.rs:9317-9337`, and commits at `native_v2.rs:9338` only after event
copy at `native_v2.rs:9304-9310` succeeds.

### Two generic handle bridges

Campaign handles need two common observations presently available only through
the sealed Level 1 handle registry. Add these generic symbols while reusing the
existing revisioned layouts verbatim:

```c
int32_t mc_native_campaign_events_copy_since(
    uint64_t handle,
    const McEventQueryV2 *query,
    McEventV2 *buffer, uint32_t capacity, uint32_t element_size,
    McEventBatchV2 *out_batch, uint32_t out_batch_size);

int32_t mc_native_campaign_presentation_frame_copy(
    uint64_t handle,
    const McPresentationFrameQueryV2 *query,
    uint8_t *index_buffer, uint32_t index_capacity,
    uint8_t *provenance_buffer, uint32_t provenance_capacity,
    McPresentationFrameInfoV2 *out_info, uint32_t out_info_size);
```

`McEventQueryV2`, `McEventBatchV2`, and `McEventV2` already define
generation/cursor/sequence/state-hash paging (`research/private/remc2-sim-rs/crates/sim-capi/include/mc_sim.h:982-1027`), and the existing Level 1 symbols demonstrate both bridges (`mc_sim.h:1377-1381`, `1508-1513`). The new functions select the campaign registry; they do
not merge handle namespaces or change sealed Level 1 symbols.

If a production audio effect needs the existing `McAudioCommandV2` sidecar,
the ABI bead MAY add campaign-prefixed sidecar lifecycle functions that reuse
that layout. It MUST NOT create a level-prefixed audio API or ask Unity to infer
simulation outcomes. Asset identity comes from the validated campaign
definition/presentation recipe.

`McCampaignEventV2` and its sidecars remain the sealed Level 2 dispatch receipt
carried by the current step signature. A later descriptor MAY project an
already-defined row kind when the meaning is exact, otherwise it emits zero
rows there and uses the generic `EffectJournal`. It MUST NOT encode new game
authority by overloading a Level 2 event kind.

### Frozen compatibility adapters

Everything after the common row copies that names a family or Day presentation
is compatibility surface, not the generic engine API: shield, invisibility,
beyond-sight, opponent, Day HUD/music/indexed frame, minimap, goats, spell
pickups, mana rows, economy rows, authored-group rows, and goat-event rows
(`mc_sim_level2.h:678-740`, `775-815`). Existing Level 2 callers keep their
behavior and layouts. Internally those functions MAY borrow the canonical
campaign state after the corresponding ownership cutover; they MUST NOT remain
a second Level 2 simulation.

The managed consumer similarly stops growing `OpenLevelN` methods and per-level
hash dictionaries. Today it pins a Level 2 file dictionary and calls
`CampaignSelection.Level2Day()` (`research/private/remc2-unity-native/Assets/Scripts/NativeCampaignSession.cs:20-51`, `95-144`). The generic consumer validates the package/definition identity once, passes the existing selection triple, and selects presentation recipes from validated descriptor data. Level 2 wrappers remain adapters until their accepted tests retire them.

## Campaign package contracts v1/v2/v3 and SourceFacts schema

### Physical package envelope

A generalized package contains the existing pointer-free owner members plus
one canonical UTF-8 JSON member named `campaign-definition.json`. The outer
`manifest.json` uses schema `mc2-owner-campaign-package/v1` and contains:

```text
schema: "mc2-owner-campaign-package/v1"
byte_order: "little-endian"
level: { external: u16, internal: u16, map_type: { id: u8, name: string } }
files: { member_name: { bytes: u64, sha256: hex32 } }
package_sha256: hex32
```

`package_sha256` uses the current algorithm: for member names in bytewise
lexical order, hash `name + NUL + lowercase_sha256 + NUL + decimal_bytes + LF`.
`manifest.json` is not a member of its own hash. The actual directory MUST be
exactly `manifest.json` plus the declared regular-file members. The current
exporter and loader already use this member identity construction and exact-set
validation (`exporter_main.cpp:1916-1930`; `native_world.rs:27165-27242`).

`campaign-definition.json` is serialized with UTF-8, sorted object keys, no
insignificant whitespace, decimal integers, and a trailing LF. Its ordinary
member SHA-256 binds its exact bytes; there is no self-hash field.

Qualified Level 2 uses `mc2-owner-campaign-package/v2` and
`mc2-campaign-definition/v2`. This is the same closed envelope plus the exact
304-byte `building-parameters.bin` owner member and its version-1
`building-parameters` SourceFact. That private fact binds the standing-building
objective's authored `par1_14` root to its bounded BLDGPRM successor chain.
Level 3 remains on the v1 envelope because it does not carry that source fact.
Qualified Level 1 uses `mc2-owner-campaign-package/v3` and
`mc2-campaign-definition/v3`: its campaign envelope adds two exact canonical
one-frame `carpet-flight-indexed-sprite/v1` payloads for the semantic Fireball
projectile slot, derived and pinned from owner TMAP 463 plus the Night/fade
palettes before package identity is computed. The wrapped owner-export identity
remains unchanged. The public semantic objective remains
`NoStandingBuildings`; raw owner selectors do not cross the MC2 adapter.

### Definition schema

The definition member has this closed shape:

```text
schema: "mc2-campaign-definition/v1"
source:
  remc2_commit: exact 40-lowercase-hex pinned revision
identity:
  external_level: u16
  internal_level: u16
  map_type_id: u8
campaign_spec:
  authored_records: MemberRef + typed record schema/count/record_bytes
  authored_behavior_rows: [AuthoredBehaviorRow]
  reset_sources:
    terrain: MemberRef + schema
    entities: MemberRef + schema/count/record_bytes
    players: MemberRef + schema/count/record_bytes
    runtime: MemberRef + schema
    feature_boundary: [MemberRef + schema]
  initial_rng: typed source field reference
  initial_activation: [TypedOperation]
  objectives: [TypedOperation]
  presentation_recipe: PresentationRecipe
source_facts: [SourceFact]
required_behaviors: [BehaviorRequirement]
```

`MemberRef` is exactly `{member: string, sha256: hex32}`. Its hash MUST equal
the outer manifest entry. Counts, record sizes, indices, offsets, relations,
and selectors are revalidated by the named compiled decoder; a descriptor
cannot manufacture an unchecked byte view.

`AuthoredBehaviorRow` is exactly:

```text
{
  authored_record: u16,
  initial_source_slot: u16 or null,
  class: u8,
  action: u8,
  model: u8,
  subtype: u8,
  initial_dispatch_sites: sorted [stable_u16]
}
```

Rows sort by `(authored_record, initial_source_slot)` and MUST equal a fresh
decode of the authored/reset members; they are an inventory and validation
index, not a second source of initial state. Keeping both model and subtype
preserves the two source vocabularies until the compiled decoder proves one is
inapplicable. The current content decoder already enforces its concrete
authored record count/width and exact 1,000 reset-slot order
(`mission_content.rs:541-577`); contract v1 makes the resulting selector
inventory explicit and hash-bound.

`TypedOperation` is a tagged, versioned data union:

```text
{ operation_kind: stable_u16, operation_version: u16, operands: closed object }
```

It is not bytecode or a behavior DSL: no jumps, loops, arbitrary expressions,
function names, addresses, or nested executable graphs are permitted. Every
kind maps to one reviewed compiled decoder/state transition; unknown kind,
version, operand, or out-of-bounds selector fails preflight. The descriptor
implementation initially admits only operation kinds proven by accepted Level
2 source evidence. `mc-iol7n` may justify additive compiled kinds for later
realms; it does not delay the descriptor-backed Level 2 fold and cannot turn
the envelope into an interpreter.

`PresentationRecipe` contains materialization selectors and asset identities
only. It may share source artifacts with a `SourceFact`, but presentation output
MUST NOT be read by Rust simulation. The present exporter labels map banks and
resolved assets under `presentation_dependencies`
(`exporter_main.cpp:2282-2339`); contract v1 makes the consumer role explicit.

### SourceFact schema

Every immutable owner value read by simulation is represented by one entry:

```text
SourceFact {
  fact_id: stable nonempty ASCII identifier,
  kind: stable_u16,
  version: u16,
  member: string,
  member_sha256: hex32,
  selector_provenance: [
    { authored_record: u16,
      source_class: u8,
      source_model: u8,
      source_subtype: u8,
      source_field: closed enum,
      selector_value: u32 }
  ],
  source_artifacts: [
    { logical_path: normalized relative path, bytes: u64, sha256: hex32 }
  ],
  lifetime: "immutable-through-session"
}
```

Entries are sorted by `fact_id`; duplicate IDs, duplicate conflicting member
claims, absolute/traversing paths, unknown kinds/versions/fields, noncanonical
ordering, or a mutable lifetime are errors. `member_sha256` MUST match the
outer file map before the kind decoder runs. The decoder validates its own
magic, version, header size, record count/order, selector closure, dimensions,
signedness, extent arithmetic, and trailing bytes.

The existing collision-mask capture is the reference implementation shape: it
selects authored class-10/model-45 `par1` values
(`exporter_main.cpp:411-425`), emits a versioned `MCM1` header and bounded
records (`exporter_main.cpp:459-486`), and records encoding plus exact source
file hashes (`exporter_main.cpp:507-525`). Rust already parses that member as a
source-exact collision bank before session construction
(`native_world.rs:27025-27042`). Contract v1 generalizes the envelope, not the
mask semantics.

### BehaviorRequirement schema

Required behavior is a sorted exact set of executable roots in the captured
post-`GenerateEvents` reset state, never a source address:

```text
BehaviorRequirement {
  dispatch_site: stable_u16,
  selector:
    ClassAction { class: u8, action: u8 }
    | ClassModel { class: u8, model: u8 }
    | TypeSubtype { type_id: u8, subtype: u8 }
    | DirectCallback { stable_callback_id: u16 },
  provenance: [authored record or predecessor BehaviorKey]
}
```

The exporter derives roots only from live reset rows and retains dormant
authored rows as an exact inventory. This is deliberate because schema v1
packages capture the state after remc2's load-time `GenerateEvents` and
`ApplyEvents` phases; treating dormant pre-generation rows as live callbacks
produced false requirements such as class-0 actions. Declaration order cannot
affect the root set. Model/subtype is a key only when the pinned source table or
constructor selects on it; a guard inside a class/action callback remains
handler logic. Later selectors are selected solely by the source-keyed compiled
registry and an unknown selector rolls the candidate step back before callback
or epilogue. Contract v1 therefore claims exact preconstruction coverage of the
reset roots plus fail-closed runtime coverage, not a package-supplied executable
transition graph.

### Fail-closed validation order

Open performs these stages before inserting a handle or mutating global state:

1. parse outer JSON with exact schema, field set, integer bounds, byte order,
   normalized member names, and zero/absent reserved values;
2. enumerate the directory and require the exact regular-file set;
3. verify every member size/hash and `package_sha256`;
4. parse canonical `campaign-definition.json`, verify its member hash, source
   commit, identity equality with the outer manifest and caller selection, and
   exact field set;
5. run each compiled typed member decoder and validate counts, record extents,
   relations, selector bounds, and topology;
6. validate every `SourceFact` reference and kind-specific payload without
   consulting presentation output;
7. validate typed initialization/objective operations;
8. compute the live post-`GenerateEvents` reset-root set and require equality
   with the declared sorted requirements, then require every root in the
   compiled registry; later runtime selectors remain exact-keyed and
   transactional at the source scheduler;
9. construct one `ResetState`, run structural invariants, and compute the
   canonical baseline receipt; and
10. only then allocate the `CampaignSession` handle and materialize optional
    presentation resources.

The current parser validates package members before metadata, relations, and
reset construction (`native_world.rs:26995-27141`); contract v1 preserves that
monotone validation shape while removing the hard-coded Level1/Level2 member
arrays.

## Contract v1: compiled behavior registry

The production representation is source-shaped and closed:

```rust
#[repr(u16)]
enum DispatchSite {
    ApplyEvents,
    UpdateEntities,
    Constructor,
    TypeSubtype,
    Direct,
}

enum BehaviorSelector {
    ClassAction { class: u8, action: u8 },
    ClassModel { class: u8, model: u8 },
    TypeSubtype { type_id: u8, subtype: u8 },
    DirectCallback { stable_callback_id: u16 },
}

struct BehaviorKey { site: DispatchSite, selector: BehaviorSelector }

struct BehaviorEntry {
    key: BehaviorKey,
    handler: fn(EntityHandle, &mut SourceWorld<'_>) -> Result<(), SimulationFault>,
    reachable_successors: &'static [BehaviorKey],
    required_source_fact_kinds: &'static [SourceFactKind],
}
```

Names are illustrative Rust spelling; the fields and ownership are normative.
The registry is a sorted `static` slice, table, or exhaustive `match`. It owns
no mutable state and performs no runtime registration. `SourceWorld` contains
disjoint borrows of canonical state domains plus immutable definition/facts;
it never contains copied terrain, entities, RNG, players, objectives, or
journals.

The scheduler owns RNG phase boundaries, prepasses, iteration domain and
direction, rescan/runagain, index validity, same-pass epilogue, and
following-pass cleanup. The handler owns only the selected transition. This
separation follows the source: `ApplyEvents_498A0` advances RNG and repeats the
complete slot scan (`Events.cpp:419-424`) before table calls
(`Events.cpp:426-470`); `UpdateEntities_57730` advances RNG, scans/rebuilds
lists (`EventsFunctions.cpp:39947-40090`), runs prepasses
(`EventsFunctions.cpp:40093-40115`), and only then calls the table
(`EventsFunctions.cpp:40116-40172`).

The cross-check extractor reads pinned remc2 source text and emits deterministic
source inventory containing site, source selector, table ordinal, source
callback identity/address token, and pinned commit. A test compares:

- implemented registry keys present at the corresponding source site;
- implemented keys absent from the source inventory (hard failure);
- source keys not implemented (explicit bounded backlog); and
- live package roots absent from the implementation (hard preflight failure),
  while later reached unknown selectors are hard transactional scheduler
  failures.

It MUST NOT emit Rust handlers, infer semantic equivalence from addresses, fold
distinct dispatch sites, or turn source addresses into package ABI. The pending
`mc-campaign-registry-probe-udemg` must supply empirical counts and may change
whether key declarations are handwritten or generated; it cannot change the
compiled/static/no-behavior-code boundary without falsifying D3.

## Contract v1: CampaignSession and ResetState carrier

### Canonical ownership

```text
CampaignDefinition                         immutable, shareable
  identity + package identity
  CampaignSpec
  SourceFacts
  validated required behavior roots

CampaignSession
  definition: Arc<CampaignDefinition>
  baseline: Arc<ResetState>                immutable frame-zero snapshot
  baseline_hash: ResetReceipt
  state: CampaignState                     only live mutable authority
  generation: u64                          lifecycle envelope, not ResetState

CampaignState
  persistent: PersistentState              empty until source evidence admits fields
  live: ResetState
  scratch: ScratchState                    derived/tick-local, never authoritative

ResetState
  flight
  source_arena                              slots, generations, free/list/map topology
  terrain
  players_and_runtime
  source_sidecars                           slot+generation keyed when required
  source_rng
  objectives
  effect_journal
```

`Arc` means immutable sharing, not shared mutable authority. A candidate step
clones `CampaignState` only; it shares the definition and baseline. A new
session initializes `live` from the baseline, then requires identical reset
receipts. `PersistentState` begins empty. A field enters it only with source
evidence that reset preserves it and with a test proving both preserved and
reset values. `ScratchState` declares validity epochs and is cleared or rebuilt
at source schedule points.

Every source-observable value has exactly one category:

| Category | Hash/reset rule | Examples/constraint |
|---|---|---|
| immutable definition/SourceFact | definition hash, never copied into live merely for access | authored bytes, immutable collision masks |
| persistent | campaign state hash, survives the named reset | none admitted until census evidence |
| resettable live | baseline/live canonical encoder and state hash | arena, terrain, RNG, players, objectives, journal |
| canonical topology | part of resettable live and its hash | generations, free stack, persistent source links/map chains |
| derived scratch | excluded from authoritative hash; explicit invalidation/rebuild | source-rebuilt class/model indexes |
| external projection | never feeds simulation | render buffers, Unity objects, parity trace |

Typed sidecars are admitted only when the value is not representable in the
source record, are keyed by slot+generation, have one writer, define reclaim
timing, and participate in the canonical encoder. A typed behavior view borrows
the arena and sidecar; it is not a second entity store.

### Step, reset, and failure atomicity

```text
step(input, output_capacities):
  validate input/generation/reserved fields
  candidate = state.clone()                 baseline and definition are shared
  scheduler.step(definition, &mut candidate)
  compute snapshot/frame/effect receipts from candidate
  prove every requested output fits and copy it
  state = candidate                         single commit point

reset():
  state.live.clone_from(baseline)
  state.scratch.invalidate_all()
  generation = generation.wrapping_add(1)
  preserve only state.persistent
```

Capacity, pointer, encoding, generation, cursor, handler, or presentation-copy
failure before commit leaves state, generation, journal, and cursors unchanged.
Reset invalidates old cursors through generation even when the baseline bytes
are identical. Repeated reset is idempotent apart from generation.

### Canonical receipts and reset/hash isomorphism

One total, versioned encoder `encode_reset_v1(&ResetState)` defines both the
baseline receipt and the live reset projection. It serializes fixed-width
integers with explicit wrapping/source widths; orders arena rows, sidecars,
objectives, and journal rows by their source-observable order; includes topology
and RNG; and rejects unrepresentable lengths. No Rust layout bytes, map
iteration accidents, debug strings, addresses, or scratch caches enter it.

During migration, the accepted public `campaign_state_hash` remains byte-for-
byte stable. A dual **observer** computes both old and canonical receipts from
the one old authority; there are never dual writers. Cutover for a carrier is
allowed only when the following focused relations hold:

1. frame-zero old receipt equals the canonical projection and
   `hash(live) == baseline_hash`;
2. a discriminating step changes every expected mutable component and neither
   immutable facts nor excluded scratch;
3. reset restores canonical live bytes exactly, preserves only enumerated
   persistent bytes, clears/rebuilds scratch, and invalidates stale generation
   and cursor queries;
4. reset followed by replay of the same input sequence produces the accepted
   state hash, terrain hash, source-arena/topology receipt, RNG receipt, ordered
   effects/audio, objective state, and presentation-frame receipt;
5. a deliberately undersized output buffer proves candidate state is not
   committed;
6. parity trace disabled/enabled produces identical production receipts; and
7. a planted omitted field changes the old observer but makes the canonical
   equality test fail.

The current hash is explicitly assembled from separate subsystem reset hashes
and Level-2 activation sets (`native_world.rs:23563-23640`), so a mere equality
of one friendly top-level hash is insufficient. The observation vector must
cover the component receipts and ordered outputs above.

`mc-campaign-state-census-dxvvd` must classify every existing field before the
first semantic ownership cut. Any unclassified or multiply classified field is
a blocker, not an invitation to guess.

## Contract v1: proposed implementation Bead graph

`mc-campaign-fold-contract-r78sq` is the named consumer. If it adopts this memo,
it can create the following leaves without rediscovering the design. `CF-*`
labels are proposal handles, not Bead IDs. Each leaf owns implementation and
discriminating tests together; each shared-carrier leaf is sequential.

Remaining Level 2 behavior work does not wait idle for the fold. Per the
operator decision recorded on `mc-campaign-fold-contract-r78sq`, existing
Level 2 Beads continue transition-style: each leaf names its dispatch site and
class/action selector, ordinary producer, schedule position, same-pass
epilogue, following-pass cleanup/consumer, narrow read/write set, ordered RNG
and emissions, and focused positive/negative route. Such a leaf MUST NOT add a
baseline/live twin, a Level-2-only route tag, a family ABI, or a new mutable
authority. Its accepted handler and receipts become inputs to CF-00/CF-03/CF-10
rather than being reimplemented by the fold graph.

| Proposal | Dependencies | Owned surface and work | Acceptance evidence | No-claim / retirement |
|---|---|---|---|---|
| FOLD-0 Sealed Level 1 internal facade (`mc-fold0-level1-facade-uf827`) | this memo only; explicit operator gate exemption | internal static adapter beside the existing Level 1 `SessionRecord` and its tests; route existing open/step/reset through the adapter with no public ABI or state change | complete existing Level 1 corpus, byte-identical replay/reset hashes, invalid-key pre-mutation negative, focused Level 2 guards, format/lint | does not migrate Level 1 or begin the semantic fold; retires only at terminal Level 1 descriptor equality |
| CF-00 Seal fold inputs and Omega | frozen pre-fold Level 2 surface plus every accepted transition-style Level 2 receipt; `mc-campaign-rw-inventory-sh828`, `mc-campaign-state-census-dxvvd`, `mc-campaign-registry-probe-udemg`, `mc-iol7n`; operator adoption | conformance/trace tests and bounded checked-in inventory only; freeze the old observation vector, exact evidence revision, state classification, registry counts, and remaining-realm capability census | two deterministic runs; planted missing state/transition/effect/capability fails; no production mutation; sealed Level 1 corpus remains green | does not close Level 2 or refactor; retires when all later leaves cite the sealed receipt set |
| CF-01 Pure campaign module extraction | CF-00 | one writer owns `native_world.rs` extraction plus new `sim-core/src/campaign/` facade; move code only, preserve visibility and call order | format, focused sim-core route/reset/hash tests, workspace lane; byte-identical Omega | no new carrier, descriptor, dispatch, or ABI semantics; retires after moved facade is canonical |
| CF-02 Package descriptor and SourceFacts v1 | CF-01; accepted Level 2 source/operation evidence; collision fact accepted | exporter schema/emission, package verifier, `campaign/spec.rs`, `campaign/source_facts.rs`; admit only proven Level 2 operation/fact kinds; no scheduler/state edits | two byte-identical Level 2 exports; exact-set/hash/identity checks; planted unknown field, source-fact mutation, traversal, and missing live-root capability reject before session construction; later unknown selectors roll back before callback/epilogue | proves Level 2 package validation and reset-root capability coverage, not later-realm schema completeness, static transition-graph completeness, behavior, or playability; retires at schema v1 consumer |
| CF-03 Static registry and drift guard | CF-01; registry probe | `campaign/dispatch.rs` plus read-only atlas extractor artifact/test; compiled entries initially delegate to existing handlers | all implemented keys match pinned site inventory; missing source key/extra Rust key/site flattening planted failures; no dynamic registration | no scheduler cutover or new behavior; extractor never emits handlers; retires when registry is build-gated |
| CF-04 Carrier shell and dual observer | CF-01; state census; CF-00 | `campaign/state.rs` and tests; introduce definition/baseline/live/persistent/scratch types and canonical encoder, but keep old authority | frame-zero/step/reset dual-observer equality; planted omitted field fails; public hashes unchanged | no ownership cutover; retire observer only after CF-09 |
| CF-05 Terrain single-owner cutover | CF-04 | canonical terrain field and all terrain borrows; delete tick-time copies only after cutover; one writer owns affected behavior call sites | source-order terrain writes/reads, reset/hash, replay, and negative stale-cache test; zero `synchronize_level2_terrain` copies remain | terrain only; no arena/RNG/journal move |
| CF-06 SourceArena identity/topology cutover | CF-05 | slots, records, generations, free stack, map/list topology, typed slot+generation views and reclaim; sequential ownership of arena writers | allocation into earlier/later slots, reuse generation, same-pass epilogue/following-pass reclaim, topology receipt, reset/replay | no scheduler rewrite beyond calls required to preserve current order |
| CF-07 RNG, flight, players, and global-runtime cutover | CF-06 | canonical source RNG and player/global carriers; remove shadows/reconciliation for this set | ordered RNG receipts at both source sites; local-player slot timing; failure-atomic step; reset/replay | no objectives/journal cutover |
| CF-08 Objectives and EffectJournal cutover | CF-07; CF-02 typed operations | canonical objective state and single ordered production journal; per-domain outputs become appends | objective following-turn visibility, exact event/audio order, cursor expiry/generation, failed-capacity noncommit | parity trace stays diagnostic; no C ABI change yet |
| CF-09 Baseline-twin deletion and canonical reset | CF-08 | remove classified `baseline_*` twins and old reset copy code; candidate clone shares definition/baseline | all reset/hash isomorphism tests; transaction clone excludes baseline; state census guard has zero orphan fields | no scheduler or ABI semantic change; retires dual observer after final accepted equality |
| CF-10 SourceScheduler cutover | CF-03; CF-06 through CF-09; rw inventory | `campaign/schedule.rs` and registry invocation; preserve separate `ApplyEvents`, `UpdateEntities`, constructor, epilogue, and cleanup sites | phase trace with RNG/prepass/rebuild/slot/rescan/epilogue/cleanup; earlier/later allocation and runagain negatives; accepted Omega | no new behavior families; existing handlers only |
| CF-11 Descriptor-backed Level 2 session | CF-02; CF-10 | replace hard-coded `MissionContentProfile`/Level2 activation selection with `CampaignDefinition`; keep sealed Level 1 adapter | production Level 2 package opens/steps/resets through descriptor; mismatch/unknown operation/fact/capability fail before handle; accepted Level 2 hashes/ABI unchanged | no Level 3 claim and no Level 1 rewrite |
| CF-12 Generic campaign ABI subset | CF-11 | `sim-capi` header/implementation/tests for D1 common semantics and the two bridges; existing Level 2 symbols remain adapters | ABI layout guards; open/reset/step/world/journal/frame positive route; reserved/generation/cursor/capacity negatives; full Rust lane and named soaks at final candidate | no managed/playable claim; no family or per-level ABI |
| CF-13 Generic managed campaign consumer | CF-12 | `NativeSim.cs`, generic campaign wrapper, package/definition validator, and the composition owner; remove growth path for per-level hash dictionaries while retaining Level 2 adapters | non-empty Level 2 EditMode route through generic open/step/reset/journal/frame; stale/mutated package negatives; accepted Level 2 presentation unchanged | no Level 3 playability; Level 2 wrappers retire only with equivalent consumer coverage |
| L2-CLOSE Final folded Level 2 gate (`mc-fi4n`) | CF-13; every required Level 2 behavior/qualification/presentation dependency | no gameplay implementation; freeze the exact folded-engine candidate, owner package, plugin, Unity identity, and run the named closure commands once | all existing `mc-fi4n` acceptance plus byte-identical pre-fold Level 2 replay/reset/state/terrain/dispatch receipts on the folded engine | this is the Level 2 close; no separate old-shape close or weakened golden |
| CF-14 Level 3 descriptor proof and shared deltas | L2-CLOSE; `mc-iol7n` census | Level 3 package descriptor plus one Bead per genuinely missing shared behavior family, each with ordinary producer-to-consumer lifecycle and disjoint handler files; final integrator owns descriptor/composition | real owner-package Rust -> C ABI -> Unity positive route, missing capability/fact negatives, replay/reset/hash, and no `Level3Session`/per-level ABI symbols | proves first generalization only; does not claim remaining campaign coverage |
| CF-15 Terminal Level 1 descriptor re-expression | capability saturation across folded Level 2 and representative Level 3+ realms; CF-14 or later evidence | Level 1 descriptor/SourceFacts/mappings onto shared compiled behaviors plus dual-observer qualification; sealed implementation remains available during comparison | byte-identical complete frozen corpus replay and fresh 182/182 matrix at one exact revision; no golden/tolerance change | mandatory end-state, not an early generalization proof; sealed path retires only after exact match |

The adopted tracker mapping is:

| Proposal | Implementation Bead |
|---|---|
| CF-00 | `mc-cf00-seal-fold-inputs-omega-iplbq` |
| CF-01 | `mc-cf01-pure-campaign-module-extraction-19t39` |
| CF-02 | `mc-cf02-package-descriptor-sourcefacts-v1-g3z5i` |
| CF-03 | `mc-cf03-static-registry-drift-guard-z34uk` |
| CF-04 | `mc-cf04-carrier-shell-dual-observer-6pxmp` |
| CF-05 | `mc-cf05-terrain-single-owner-cutover-7pws6` |
| CF-06 | `mc-cf06-source-arena-topology-cutover-0fyke` |
| CF-07 | `mc-cf07-rng-flight-player-runtime-cutover-8yf18` |
| CF-08 | `mc-cf08-objectives-effect-journal-cutover-mrdri` |
| CF-09 | `mc-cf09-baseline-twin-deletion-canonical-reset-hj11g` |
| CF-10 | `mc-cf10-source-scheduler-cutover-c9q74` |
| CF-11 | `mc-cf11-descriptor-backed-level2-session-yg8j2` |
| CF-12 | `mc-cf12-generic-campaign-abi-subset-hs687` |
| CF-13 | `mc-cf13-generic-managed-campaign-consumer-reneq` |
| L2-CLOSE | existing `mc-fi4n`, now blocked by CF-13 |
| CF-14 | `mc-cf14-level3-descriptor-shared-deltas-nrs2v` |
| CF-15 | `mc-cf15-terminal-level1-descriptor-reexpression-inim6` |

The tracker encodes CF-05 through CF-11 as one blocking single-writer chain.
CF-10 also has the table's explicit CF-03, CF-06-through-CF-09, and read/write
inventory edges. CF-00 is blocked by the contract-adoption Bead and every
frozen Stage-A receipt. CF-14 is blocked by `mc-fi4n` and the remaining-realm
census. CF-15 is deferred until a later capability-saturation receipt is named;
CF-14 alone proves only the first generalization. `br dep cycles
--blocking-only --json` reported zero active cycles when this mapping was
created.

Every created Bead packet MUST copy its prerequisite receipts, exact owned files
or logical carrier, ordinary production route, following-pass consumer/cleanup,
focused command, negative, and no-claim from this table. CF-05 through CF-11
share central carrier/scheduler surfaces and therefore form a single-writer
chain. CF-02 and CF-03 may run in parallel only after CF-01 leaves disjoint
`spec/source_facts` and `dispatch` files and reservations confirm no atlas or
exporter overlap.

The sealed Level 1 corpus/hash invariant is an acceptance condition on every
semantic CF candidate, even where the table omits it for space. It detects
shared-substrate regressions; it never substitutes for the Level 2 Omega named
by that leaf. `mc-fi4n` runs only after the folded Level 2 engine and consumer
exist, so the fold and Level 2 closure are one candidate rather than two gates.

### Missing research inputs at this revision

No dependent research artifact landed before this contract was written:

- `mc-campaign-rw-inventory-sh828` is in progress. Until it lands, handler
  read/write/RNG/emission sets and source-phase coverage remain provisional.
- `mc-campaign-state-census-dxvvd` is in progress. No semantic carrier cut may
  start without its exact field classification.
- `mc-campaign-registry-probe-udemg` is in progress. D3 selects compiled
  registry plus cross-check, but the drift counts and declaration ergonomics
  remain unknown.
- `mc-iol7n` is in progress in parallel with remaining Level 2 work after the
  operator removed its Level 2-completion dependency. The final realm, Level 3
  objective operations, remaining SourceFact kinds, and transitive capability
  closure are intentionally not invented here.

If any artifact lands before `mc-campaign-fold-contract-r78sq` adopts the
contract, that parent MUST reconcile its results with D1-D5 and exercise the
named falsifier. Silence is not acceptance.

### Adoption reconciliation at the implementation-graph revision

`mc-campaign-fold-contract-r78sq` adopted contract v1 on 2026-08-21 after the
following receipts landed:

- the API/seam memo itself at `3a73019c` and the sealed Level 1 facade at
  `0f43caf6`, independently accepted by `mc-fold0-level1-facade-uf827` at
  `be739f7b`;
- the state census `mc-campaign-state-census-dxvvd` at `7bf3862`, which
  classified 151 outer, 57 economy, and 89 Nyphur fields and reconciled 127
  retained baseline/live pairs plus one retired pair without an orphan or
  multiply classified state cell;
- the registry probe `mc-campaign-registry-probe-udemg` at `0a0881f0`, whose
  deterministic pinned-source inventory contains 17 class rows, 486 active
  source keys, 127 matching Rust keys, zero implemented keys absent from the
  source, and 359 explicit source-only backlog keys;
- the remaining-realm census `mc-iol7n` at `00520df3`: external Levels 3
  through 24, all three map types, 22/22 byte-identical twin package trees,
  330 files per tree, and 44/44 strict package validations. Independent
  adoption verification is comment 511 on that Bead; and
- the qualified empty-collision package decision
  `mc-pkg-v5-no-collision-ugou9` at `6e3aeb70`.

The last receipt exercises D1 and does not falsify it: every remaining realm
fits the existing external-level/internal-level/map-type identity plus the
validated package identity; no additional identity-critical caller input was
observed. It exercises D2 without admitting executable content: the observed
delta is bounded typed tuple/objective selector values and immutable
source-fact selector/artifact identities. It exercises D3 without requiring a
runtime registry or generated behavior: the large source-only set remains an
explicit capability backlog keyed by source site and selector. The state
census does not falsify D4. The dependency graph above is the D5 response and
keeps every semantic ownership change behind an old/new observation relation.

The empty-collision decision is explicitly blessed as part of contract v1.
For a qualified realm such as Level 8 with exporter-recorded zero authored
class-10/model-45 rows, the v5 package MUST carry an explicitly empty typed
collision member. An absent member, an empty member with authored rows, or a
nonempty member without authored rows rejects. The 15-test exporter/census
lane independently rerun during adoption includes the positive zero-row case,
both count/member mismatch directions, absent-member rejection, and census
cross-check. Level 1 and Level 2 package identities remain unchanged by that
decision.

The read/write inventory needs one narrower qualification. The generator and
schema from `mc-campaign-rw-inventory-sh828` are deterministic, but neither its
closure artifact at `03723bd4` nor refresh `1d28bf83` is an exact-revision
receipt: independent comment 510 shows that two fresh runs at the declared
`bdc545ca` revision produce 52 transitions, 13 route kinds, and 218 open
questions, while the checked artifact contains 54/14/224 and hashes an
uncommitted later Fireball tree. Contract v1 therefore adopts the inventory
method and its conservative read/write/RNG/emission vocabulary, not that stale
artifact hash. This is not allowed to leak into a semantic migration:
CF-00 (`mc-cf00-seal-fold-inputs-omega-iplbq`) MUST regenerate the inventory
twice from one exact current committed revision, enumerate every accepted
transition-style receipt, and seal that fresh result in Omega before CF-01 can
start. The blocking edge makes this a hard gate rather than a waived defect.

The operator sequencing decision remains the hybrid path recorded in comments
453 and 502 on the adoption Bead: remaining Level 2 behavior lands
transition-style; CF-00 through CF-13 fold the accepted engine; `mc-fi4n` is
the sole Level 2 close on the folded engine; Level 3 is the first generalized
descriptor proof; and terminal Level 1 re-expression waits for capability
saturation and exact frozen-corpus plus 182/182 equality. Adoption authorizes
only the created Beads and does not itself mutate production, close Level 2,
or amend sealed Level 1.

## Evidence at the measured revision

### Rust shape

- `Level2ResetSession` interleaves outer baseline/live fields, nested reset
  sessions, activation sets, and journals (`native_world.rs:16692-16810`).
- `synchronize_level2_terrain` clones the outer terrain into spell, ordinary,
  and economy domains before handlers run (`native_world.rs:20078-20086`).
  Those copies are mutable projections, not ordinary immutable render views.
- `step_level2_source_pass_with_player` performs Level-2-specific prepasses,
  constructs a numeric-tag route vector, sorts it by source slot, and dispatches
  it (`native_world.rs:21728-21910`).
- `step_outer_turn` owns player/RNG/pass/objective ordering
  (`native_world.rs:23950-24008`), while reset restores outer and nested
  baseline fields manually (`native_world.rs:24010-24063`).

### ABI and content-selection shape

- The campaign record is an explicit Level1/Level2 enum
  (`native_v2.rs:7954-7965`); open branches on `is_level2`
  (`native_v2.rs:8294-8349`), reset only handles the Level 2 variant
  (`native_v2.rs:8371-8388`), and step rejects the Level 1 variant
  (`native_v2.rs:9237-9243`). The generic names do not yet imply generic
  semantics.
- The safety transaction clones the entire `Level2ResetSession`, creates all
  bounded outputs, and then commits (`native_v2.rs:9282-9339`). Separating the
  immutable baseline preserves failure atomicity while removing it from the
  candidate clone.
- `MissionContentProfile::from_metadata` recognizes exactly two triples
  (`mission_content.rs:440-458`), and `MissionContentTemplate::decode` still
  branches to Level-2-specific stage variables and activation plan
  (`mission_content.rs:515-606`).
- The managed wrapper pins a Level 2 package dictionary and calls the one
  Level-2 selector (`NativeCampaignSession.cs:20-51`, `95-144`). That is a
  correct current gate and a measured per-level growth point.

### remc2 source topology

- `ApplyEvents_498A0` in pinned `Events.cpp:410-540` advances the global RNG,
  repeatedly scans slots 1 through 999, and selects callbacks from
  `str_D4C48ar[class].dword_10[action]`. Its `runagain` decision is part of
  the surrounding scheduler, not a generic collection loop. Each true
  `runagain` restarts the complete slot scan, so a remembered-row follow-up
  walk is not generically equivalent.
- `UpdateEntities_57730` in pinned `EventsFunctions.cpp:39927-40184` advances the
  RNG, rebuilds several derived linked indexes in source-slot order, runs
  prepasses, then scans the same arena and dispatches through the class/action
  table.
- `str_D4C48ar` is a 17-row static class table at
  `EventsFunctions.cpp:2060-2078`. Model and subtype checks often remain inside the
  selected callback. A level number is not the primary behavior key.
- `sub_10C80(a2 = 0)` reads `MSPRD0-0` width, height, and signed pixel data at
  `EventsFunctions.cpp:4086-4100` when resolving a live collision/effect path.
  Those pixels therefore influence simulation.
- The current exporter documents `presentation.bin` as 347 particle
  parameters, 504 sprite selectors, and 157 entity-parameter records. It does
  not carry the relevant MSPRD pixels. The Unity asset cache decodes MSPRD,
  but Unity cannot become the authoritative source for Rust simulation.

## The mathematical model

The most useful formalization is a constrained directed hypergraph with a
minimum-description-length term. The protein analogy is helpful only after
the variables are made explicit.

Let the vertices be:

```text
V = mutable state cells
  + immutable package/source facts
  + behavior transitions
  + scheduler phases
  + ABI/presentation projections
```

For each source transition `e`, record a hyperedge:

```text
e = (R_e, W_e, G_e, J_e, O_e)
```

where `R` is its read set, `W` its write set, `G` its ordered RNG consumption,
`J` its ordered event/audio emissions, and `O` its schedule predecessors and
successors. This is a richer and more faithful graph than a call graph.

First define a legal candidate as a representative/ownership map plus a module
partition whose old and new states satisfy the declared observation relation.
Reject candidates that violate any fidelity constraint. Among legal candidates,
compare the raw objective vector:

```text
C(P) = [duplicate mutable owners,
        synchronization frequency and volume,
        weighted mutable and ordered hyperedge cuts,
        fixed-codec boundary information per source step,
        handler exposed/touched mutable surface,
        public contract churn,
        measured edit contention,
        dispatch/runtime/memory cost,
        expected compile cost,
        migration cost,
        description length,
        unresolved proof obligations]
```

Use Pareto dominance, explicit budgets, and lexicographic priorities. Do not
invent exchange rates between, for example, one millisecond of runtime and one
ABI consumer. A scalar is admissible only when its units, decision-maker, and
sensitivity are recorded. The hard constraints are:

- fixed-width and wrapping arithmetic;
- source-slot, pass, and `runagain` order;
- entity allocation, reclamation, generation, and linked-index semantics;
- RNG draw count and order;
- event and audio order;
- canonical reset and hash semantics;
- package identity and fail-closed validation;
- existing C ABI layouts, cursor binding, and error behavior; and
- Unity's non-authoritative role.

The MDL term is operational, not philosophical:

```text
description_length = shared_engine_code
                   + sum(per_level_descriptor_bytes)
                   + sum(per_level_special_case_code)
```

The desired trend is for the third term to approach zero. A new level should
normally add one validated descriptor and no new runtime state type. It adds
code only when its source census reaches a behavior capability the engine does
not yet implement.

There is deliberately no balance term forcing similarly sized modules. A
cohesive 4,000-line module can have lower boundary and migration cost than
twenty 200-line modules with broad mutable interfaces.

### Translating the protein metaphor

- Exact scheduling and arithmetic are the covalent backbone: they may cross a
  file boundary but may not be reordered.
- Strongly interacting handlers and state form a hydrophobic core: keep them
  close enough that their mutable protocol stays small.
- Duplicate terrain/entity/RNG owners are frustrated bonds: every tick spends
  work reconciling them and creates another divergence path.
- A handler's measured semantic footprint or dependency frontier is its
  read/write/order/identity/fact surface. Calling this a Markov blanket would
  require a probabilistic model and a conditional-independence proof that this
  architecture does not define.
- A level package is a ligand: it selects initial conditions and required
  capabilities; it does not grow a new organism.
- A preferred fold is a feasible nondominated candidate with low measured
  boundary information and migration cost, not the prettiest directory tree.

## Proposed logical fold

```text
CampaignDefinition
  CampaignSpec                 immutable identity, authored records, objectives
  SourceFacts                  immutable owner facts consumed by simulation
  required behavior inventory  validated, fail closed

CampaignSession
  definition: Arc<CampaignDefinition>
  baseline: Arc<ResetState>    immutable reset snapshot, shared by candidates
  state: CampaignState         the only live authoritative state

CampaignState
  persistent                   only state proven to survive source reset
  live: ResetState
    flight
    source_arena               slots, generations, free stack, map links
    terrain
    players and global runtime
    source sidecars            only facts not representable in entity records
    source RNG
    objective state
    effect journal
  scratch                      derived indexes and tick-local work

SourceScheduler
  explicit DispatchSite
  source-specific iteration domain
  RNG/pass prologue and prepasses
  derived-index rebuild where source rebuilds it
  exact dispatch
  runagain/rescan rules
  postpasses
  objective/HUD/presentation projection

BehaviorRegistry
  static dispatch-site + class/action routing
  source-faithful model/subtype branches inside handlers
  no mutable ownership

External projections
  EffectJournal                 stable production output through the ABI
  ParityTrace                   optional diagnostic evidence, never authority
```

Names above are logical responsibilities, not permission to create every file
immediately.

## Ownership laws

| Fact | Canonical owner | Derived views allowed | Forbidden duplicate |
|---|---|---|---|
| entity record and liveness | `SourceArena` | typed borrowed views; slot/generation handles | per-domain entity copies |
| free-list/generation state | `SourceArena` | diagnostics | a domain-local allocator |
| source RNG | `CampaignState` | read-only receipt after a step | one RNG per behavior domain |
| terrain | `CampaignState` | immutable render snapshot or narrow mutable borrow | copied terrain in spell/economy state |
| class/model linked lists | scheduler-owned derived indexes | rebuilt/indexed views | separately authoritative entity lists |
| player/global runtime | `CampaignState` | typed borrowed views | shadow player structs reconciled later |
| objectives | objective state machine | HUD projection | Unity-side victory logic |
| emitted effects/audio | `EffectJournal` | cursor-paged ABI projection | per-domain journals merged after ordering |
| collision sprite facts | `SourceFacts` | immutable borrowed masks | Unity cache as sim authority |
| reset baseline | `CampaignSession::baseline` | immutable comparisons/hash | a baseline twin for every live field |

Typed sidecars are acceptable when remc2 state lies outside the 176-byte
entity record or when Rust representation requires it. They must be keyed by
slot plus generation, have one owner, define reclamation behavior, and enter
the canonical hash/reset contract.

## Behavior is a transition, not an object hierarchy

A useful conceptual signature is:

```text
H[dispatch_site, class, action](slot, SourceWorld)
    -> Result<(), SimulationFault>
```

The scheduler, not the handler, owns visit order, outer `runagain` rules, RNG
phase boundaries, and derived-index rebuilds. The handler mutates canonical
state and appends to the single ordered journal.

`SourceWorld` is one fixed borrow view over a small set of canonical domains:

```text
SourceWorld {
    arena,
    terrain,
    players,
    ledgers,
    objectives,
    journal,
    source_facts,  // immutable
    ...
}
```

Do not manufacture a bespoke context type for every handler. Across hundreds
of source callbacks that would create a second combinatorial API and a new
shared-file contention point. The dependency frontier remains an analysis and
audit tool: measure which fixed domains each handler actually touches, then extract
narrow pure stepping functions where that improves testing or borrowing.
`SourceWorld` itself stays small, contains no duplicate state, and must not
grow into an unstructured facade with unrelated services. Rust field
destructuring can create disjoint borrows; hot-path dynamic trait objects are
unnecessary.

Static routing uses a sorted compiled table or exhaustive `match`; D3 permits
either representation and forbids runtime registration. The pending probe may
decide whether key declarations are handwritten or generated, but it cannot
generate handlers. Both static representations preserve source-shaped dispatch.

`BehaviorRegistry` is a responsibility, not necessarily one rectangular
table. `(class, action)` is not assumed globally unique: `ApplyEvents_498A0`,
`UpdateEntities_57730`, constructor dispatch, and subtype dispatch are distinct
source sites until evidence proves otherwise. Preserve those selection moments
instead of flattening semantically different tables merely to obtain one
generic registry API. Model remains a branch inside a handler unless a source
table actually indexes by model.

## Source arena versus derived indexes

The arena owns slot identity, records, and any allocator/list topology whose
order persists and affects later behavior. It does not imply that every query
must scan all 999 live slots. remc2 rebuilds some class/model linked indexes at
a specific point in `UpdateEntities_57730`; the Rust scheduler may do the same.

Only source-rebuilt indexes are caches with explicit validity intervals:

```text
arena mutation -> index invalid
scheduler rebuild point -> index valid for the documented phase
next relevant arena mutation -> index invalid
```

This preserves performance and source traversal without creating a second
authority. A free stack, persistent list head, insertion link, or map-chain
link is canonical state when the source later observes its exact topology.
Tests should distinguish source-slot order, forward or reverse scans,
source-linked order, event-list order, and ordinary vector order because they
are not interchangeable.

## Packages and source facts

The current word `presentation` is too weak for owner data that changes
simulation. Split concepts, not necessarily files, into:

- `CampaignSpec`: level identity, authored entities, initial activation,
  players, objectives, environment/presentation recipe, and required
  capabilities;
- `SourceFacts`: immutable decoded owner facts read by the simulation, such as
  collision-relevant MSPRD dimensions and signed masks; and
- presentation materialization: textures, atlases, meshes, audio, and Unity
  resources that cannot feed authoritative state.

The closure may bind the same owner artifact for both simulation facts and
presentation materialization, but each consumer gets a purpose-specific,
hashed schema. Rust does not read Unity output.

Membership in `SourceFacts` requires evidence that the source does not mutate
the fact during load, play, or reset. Data that originated in an owner file but
is later mutated belongs in `ResetState` or proven persistent state instead.

## Effects are not the parity trace

`EffectJournal` is stable production output: ordered audio, UI, presentation,
and other externally consumed effects. It can be cursor-paged through the C
ABI, but it is not world authority.

`ParityTrace` is optional diagnostic evidence: phase, slot, dispatch key, RNG
receipt, allocation/reclamation, and selected state hashes. It may be compiled
or enabled only for verification. It must not become the replay database, the
Unity protocol, or a second event-sourced simulation.

## Objective and level seams

The level descriptor should describe initial conditions and source-authored
objective structure. It should not embed arbitrary executable scripts. Start
with typed objective operations actually observed across Levels 1 through 3;
do not invent a general graph language before source evidence requires one.

Unknown level identity, unknown objective operation, missing source fact, and
missing behavior capability all fail closed before play. Runtime failure on an
unreachable dynamic behavior remains a safety net, not the preferred package
validation strategy.

## Physical Rust topology

The eventual topology may resemble:

```text
sim-core/src/campaign/
  mod.rs          facade and stable internal inventory
  spec.rs         validated immutable campaign definition
  source_facts.rs immutable simulation inputs
  state.rs        canonical state, initial snapshot, reset, hash
  source_arena.rs slot/generation/free-list/map-link ownership
  schedule.rs     exact phases, scans, rebuilds, runagain
  dispatch.rs     static dispatch-site/class/action routing and validation
  journal.rs      ordered production effect outbox
  objectives.rs   typed source-shaped objective state

sim-core/src/behaviors/
  economy.rs
  castle.rs
  spells.rs
  creatures/...   only where measured call/state/churn seams support it
```

This is a target map, not a file-creation checklist. Physical cuts follow call
clusters, mutable-state cuts, and co-change evidence. The preferred first
mechanism is an intra-crate directory module with a facade and restricted
visibility. A workspace-crate split has no measured justification and risks
lost inlining, larger public surfaces, and slower integration.

## Migration sequence

### Now: facade and transition-style Level 2

1. Land `mc-fold0-level1-facade-uf827` exactly as the internal sealed adapter
   above. It is the sole fold-contract exemption.
2. Continue remaining Level 2 behaviors through their existing Beads, one
   source transition at a time. Record dispatch key, reads, writes, RNG,
   emissions, producer, epilogue, and following-pass consumer. Add no baseline
   twin, Level-2-only route, family ABI, or second authority.
3. Finish the read/write inventory, state census, and registry probe. The
   operator-accepted parent turns them and this memo into the semantic fold
   graph.
4. Keep the sealed Level 1 corpus/hashes green after every accepted change.

### Semantic fold and Level 2 closure are one candidate

Follow CF-00 through CF-13: seal the observation vector; separate pure moves
from semantic changes; validate the Level 2 descriptor/SourceFacts; cut over
terrain, arena/topology, RNG/runtime, objectives/journal, and baseline ownership
one at a time; extract the literal scheduler; then generalize the existing
campaign ABI subset and managed consumer.

Every semantic step keeps a real Level 2 production route and its named Omega
green, plus the standing Level 1 regression corpus. Temporary adapters have a
removal leaf and never become another writable source. Level 2 finally closes
through `mc-fi4n` on that folded engine with byte-identical pre-fold receipts;
there is no preliminary old-shape closure.

### Level 3 is the first generalization proof

After the folded Level 2 close, run the remaining-realm census and bring Level
3 through the generalized path: export and validate its package, diff required
behavior/SourceFacts/objective kinds against the compiled capabilities,
implement only missing shared transitions, add its descriptor, and prove the
real Rust -> C ABI -> Unity route. Reject any proposal that needs a
`Level3Session`, parallel source pass, or per-level ABI.

Repeat that census/diff/behavior/descriptor/consumer loop until shared
capabilities saturate. Then perform the terminal Level 1 descriptor
re-expression and retire the sealed path only after byte-identical corpus replay
and the fresh 182/182 matrix.

## Success metrics

Metrics are engineering signals, not delivery substitutes:

- exactly one mutable owner for terrain, arena records, RNG, player runtime,
  objectives, and journal ordering;
- zero tick-time copy-back synchronization between behavior domains;
- reset expressed as one canonical snapshot restoration, apart from explicitly
  external session handles;
- no `LevelNSession` type for `N >= 3`;
- no `mc_*_level_N_*` ABI growth unless the operation is genuinely unique at
  the presentation contract;
- one source phase trace that identifies prepasses, index rebuilds, slot
  visits, rescans, RNG draws, allocations, and emissions;
- new-level code delta equals only missing shared behavior, not a repeated
  mission shell;
- unknown descriptor/capability/source-fact negatives fail before mutation;
- unchanged Level 2 replay/hash, ABI observations, and focused performance
  remain within measured bounds after each semantic migration; and
- sealed Level 1 corpus/hashes remain byte-identical after every semantic fold
  step, followed by terminal descriptor-path byte identity and 182/182 before
  the sealed adapter retires;
- `mc-fi4n` closes Level 2 on the folded engine rather than certifying an
  abandoned old-shape candidate; and
- the typical handler's measured read/write set touches a small fraction of
  `CampaignState`; any broad exception carries source evidence.

Do not optimize line count, module count, commit count, test count, or Bead
count. Countermetrics are playable coverage, conformance depth, runtime cost,
compile resources, and remaining duplicate authority.

## Mystery register

These are engineering unknowns, not permanent process items. Each retires when
its decisive evidence is captured next to the implementing code or package
schema.

| Mystery | Why it matters | Smallest decisive evidence |
|---|---|---|
| Exact class/action/model/subtype, objective, map-type, and source-fact census for every remaining qualified owner realm | sizes the real remaining behavior surface and tests whether Level 3 is the best first generalization proof | export every remaining owner realm twice, require byte-identical packages, and emit a bounded per-level tuple/capability diff without running the original executable |
| Exact MSPRD selector, signed-pixel decoding, resolution branch, and bounds for the current model-45 collision | blocks exact positive Level 2 behavior and defines `SourceFacts` schema | trace the pinned `sub_10C80` branch and decode the named owner selector in exporter tests; compare exact admitted/rejected pixels |
| Exact `ApplyEvents_498A0` `runagain` semantics for allocations, deaths, and slot reuse in the same outer turn | determines scheduler control flow and generation safety | construct a Rust source-shaped microtrace for allocation into earlier/later slots and compare against pinned source flow |
| Which source pre/postpasses and cave/map-type gates are simulation-relevant beyond Level 2 | a generic slot dispatcher is incomplete if it omits the surrounding phase schedule | inventory each function around `UpdateEntities_57730`, map it to reached levels and state effects, and add only the reached phases to the scheduler trace |
| Complete inventory of mutable source globals and auxiliary arrays outside entity records | determines legitimate sidecars and canonical hash/reset | static read/write inventory for the callbacks reached by Levels 2 and 3, classified as immutable, session state, derived index, or presentation-only |
| Whether each typed Level 2 entity vector is authority, sidecar, or projection | determines what can collapse into `SourceArena` | for every vector, map its fields to entity bytes or external source globals and identify every writer/read ordering |
| Exact behavior key closure for dynamically spawned rows | package preflight cannot validate only authored starting rows | inventory constructor/action transitions and allocations reachable from each implemented class/action; fail a planted missing-handler case |
| Empirical drift and ergonomics of the compiled registry plus generated cross-check selected by D3 | decides whether key declarations remain handwritten or may be generated without generating behavior | prototype a read-only table extractor and compare its complete key/address inventory with a small explicit Rust registry; do not generate behavior code |
| Objective mechanism commonality across Levels 2 and 3 | decides whether typed objective operations suffice | export Level 3 objective/activation facts and map each transition to source functions before designing a general graph |
| Exact boundary between simulation source facts and presentation assets beyond MSPRD | prevents Unity from becoming accidental authority | scan all Level 2/3 reached callbacks for reads from TMAP/MSPRD/TABLESN/audio/palette data and classify each read by state effect |
| Hash compatibility while state moves from nested structs to canonical carriers | required for safe semantic migration | compute old/new canonical receipts in a dual-observation test without dual authority; require identical semantic fields and replay hashes |
| Performance of `match` versus static function-pointer dispatch and typed borrow construction | avoids abstract hot-path claims | benchmark the same source-slot trace on the same revision and machine; compare runtime, code size, and compile resources |

## Rejected attractive designs

- **One module per source callback:** too granular; it raises boundary entropy
  and agent navigation cost without creating independent ownership.
- **One module per level:** repeats the current Level 2 shell across the
  remaining campaign and
  hides shared behavior.
- **ECS:** remc2's stable slots, reuse, linked indexes, and global pass order
  are primary semantics; adopting an ECS would require proving its iteration,
  storage, and generation behavior rather than advancing parity.
- **Runtime plug-ins or scriptable behavior graphs:** capability discovery is
  useful, runtime registration is not. All preservation behavior is pinned,
  compiled, and source-reviewed.
- **A giant mutable handler context:** reduces compiler friction by erasing the
  very seams the refactor is meant to establish.
- **Exporter-generated Rust behavior:** the exporter carries data and evidence,
  not executable reconstruction logic.
- **Crate-per-domain:** no current deployment, compile, or ownership boundary
  justifies the API/performance cost.
- **Ungated all-at-once fold:** combines physical moves, authority changes,
  scheduler replacement, ABI work, and managed migration without the
  carrier-by-carrier observations required by CF-00 through CF-13.
- **Never refactor:** makes every later level pay the same integration tax and
  compounds the existing duplicate-authority risk.

## Final architectural test

Ask one question whenever a new source behavior, field, or artifact enters the
engine:

> Is this immutable input, canonical mutable state, a derived view, a scheduled
> transition, or an external projection?

If it occupies two categories, the boundary is not yet understood. If it has
two mutable owners, the fold is wrong. If adding a level requires copying the
campaign shell, the fold is not finished.
