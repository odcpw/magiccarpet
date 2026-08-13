# Level 2 throughput refactor pipeline

Status: architecture decision for the preservation track at
`143769bdb977f231adb1b94cd72459950d70c638`. This document governs only the
next neutral extraction and the four open authored class-5 packets. It is not a
standing decomposition program.

## Decision

Keep advancing the Nyphur lane. Before dispatching `mc-38gs.1` through
`mc-38gs.4`, make one behavior-neutral extraction of the existing authored
class-5 handler packet into a private `native_world/level2_class5/` directory.

The extraction is a real throughput change because MagicCarpet coordinates
writers by file reservation and all four packets currently name the same
44,512-line file. It is not a new simulation architecture. The shared source
slot arena, allocator/free order, RNG consumption, activation schedule,
cursor, hash/reset ordering, and C ABI remain one authority and one serialized
integration surface.

After the extraction, parallelize model-owned transition bodies and focused
tests. Serialize the short shared-carrier edits that a packet actually proves
necessary. Tracker readiness never overrides that ownership rule.

Do not modify pinned remc2. It remains the read-only specification at
`6806c7204d17220ecc0430759f784aea8ee2ddb2`.

## Why this cut, and no larger one

Current evidence:

- `native_world.rs` is 44,512 lines and 1.8 MiB. Its single test module starts
  at line 26,316 and contains 181 test functions.
- `native_v2.rs` is 32,968 lines, but none of the four class-5 packets currently
  requires an ABI change. Splitting it would not open their writer lanes.
- The existing Nyphur and dynamic-economy child modules demonstrate that Rust
  child-module privacy can move implementation and tests without widening the
  crate's public API.
- All four ready `mc-38gs` leaves name `native_world.rs`; `mission_content.rs`
  has no current authored-class-5 behavior. Generic allowed-scope text is not
  evidence that a file must change.
- The class-5 models have distinct remc2 transition and constructor bodies,
  but share NewEvent slot order, the free stack, source cursor, RNG, hash/reset,
  and scheduler. A model tag is therefore a handler seam, not an ownership
  seam.
- Nyphur and dynamic-economy child files were created on 2026-08-12. Their raw
  historical co-change counts are too young to establish a causal throughput
  improvement; future analyses must age-normalize their observation windows.

Candidate comparison:

| Candidate | Writer throughput | Semantic risk | Decision |
|---|---:|---:|---|
| Leave all behavior in `native_world.rs` | one class-5 writer | lowest immediate churn | loses to the observed reservation bottleneck |
| Move only the 18K-line test module | unchanged | large blame churn | reject; it moves no production writer surface |
| One `level2_class5.rs` file | one class-5 writer | low | useful for comprehension, insufficient for the four packets |
| Private class-5 directory with a frozen shared spine and model handlers | model-body lanes plus serialized spine integration | low if textually neutral | **chosen** |
| Broad authored-entity subsystem, trait registry, ECS, or SourceArena rewrite | unknown | high, speculative | defer until two delivered levels prove a consumer |

The selected point is Pareto-better only if the post-extraction packet ledger
shows genuinely smaller, mostly disjoint edit surfaces. If it does not, the
four feature packets remain serialized; no abstraction is added to rescue the
plan.

## Protected behavior

The neutral move must preserve these observations exactly:

- fixed-width and wrapping arithmetic;
- source-slot and source-list identity;
- entity-pass, activation, constructor, and reclaim order;
- RNG draw count and order;
- event and audio order;
- allocation failure and no-target branches;
- campaign reset/replay isolation;
- every hash byte and domain label;
- internal and C ABI layouts and error behavior;
- Level 1 identities and all accepted Level 2 behavior.

These are hard feasibility constraints, not weighted preferences. File size,
compile time, and parallel reservations matter only among behavior-preserving
candidates.

## Target seam and authority map

The initial directory is private to `native_world`:

```text
native_world.rs
  owns session composition, Level2EconomyState storage, schedule/pass order,
       source arena/free stack, activation call sites, reset/hash composition,
       C-ABI-facing observations

native_world/level2_class5/mod.rs
  owns the existing class-5 carrier/dispatcher packet, shared constructor
       parameterization, fixed model dispatch, and common cursor tail
  does not own a second arena, scheduler, RNG, reset, or hash

native_world/level2_class5/model3.rs
  owns existing model-3 transition bodies and focused tests

native_world/level2_class5/model9.rs
  owns existing model-9 transition bodies and focused tests

native_world/level2_class5/archer.rs
native_world/level2_class5/trader.rs
  are created by their feature packets when real behavior moves into them;
  the neutral extraction must not create placeholder implementations
```

The existing `native_world::Level2AuthoredClass5Source` path stays stable via a
private re-export if the carrier type moves. The parent keeps the authoritative
field storage, allocator/free order, dynamic-arena registry, activation order,
hash/reset call order, and public ABI. A child helper may append bytes to the
parent-supplied hasher only at the existing exact position; it does not choose
the hash schedule.

The parent dispatch arm may delegate to a child helper. Moving a guard or
predicate inward is a semantic transformation, not a textual move, and must
not be hidden in the neutral extraction. Either move it token-for-token with
its containing body or leave it in the parent for the feature packet.

## Pipeline

### 1. Keep the active capability lane moving

`mc-sq58.1` continues in `level2_nyphur.rs`. It has no semantic dependency on
the class-5 refactor. If it proves a necessary `native_world.rs` edit, Agent
Mail reservations serialize that short overlap; the Beads graph does not
invent a false dependency.

### 2. Land one neutral, independently useful extraction

The refactor Bead moves only current behavior and its focused tests. It must:

1. create the private class-5 shared spine;
2. move current model-3 and model-9 bodies into real model files;
3. keep absent archer/trader behavior absent rather than scaffolding it;
4. keep composition, arena authority, reset/hash ordering, and the ABI in the
   parent;
5. produce a post-extraction edit ledger for all four feature packets.

The extraction retires when these files are compiled production code and the
four packet ledgers name their remaining shared edits. There is no follow-up
architecture Bead.

### 3. Dispatch only the parallelism the ledger proves

- `mc-38gs.2` model 9 and `mc-38gs.4` Trader are the best first parallel pair
  if their ledger contains only model files plus bounded carrier/hash lines.
- `mc-38gs.1` model 3 may require new child state or links.
- `mc-38gs.3` archer is most likely to require a shared child arena kind,
  dispatch arm, hash/reset, and census integration.
- Any packet needing `level2_class5/mod.rs` or `native_world.rs` reserves that
  shared surface and lands its carrier integration before or after another
  shared writer. Its longer model implementation may proceed independently.
- Two writers never edit the shared spine merely because their model files are
  disjoint.

The intended maximum is several model-body writers, not four independent
simulation authorities.

### 4. Use delivered levels to shape Levels 3-25

After Level 2 closes, `mc-1agi` remains the consumer of the proven campaign
seams. Level 3 is first treated as a validated data/content delta. Repeat this
extraction pattern only when a second real level or a new multi-leaf collision
proves the same seam. Do not pre-build a plugin system, ECS, scripting DSL, or
generic source arena for hypothetical levels.

## Refactor verification contract

Green tests and an unchanged test count are necessary but not sufficient to
establish isomorphism. The neutral Bead requires all of the following:

1. Record the exact pre-move Level 2 profile result and sim-core test census.
2. Keep moved production spans and moved test bodies token-identical. Review
   `git diff --color-moved=dimmed-zebra`; isolate any non-move change and reject
   it from this Bead.
3. Preserve the exact public path and compile every existing consumer.
4. Run the moved focused class-5 test and the crossing production composition
   controls, including group-20 archer materialization and group-22 source-order
   materialization.
5. Run format, focused correctness/suspicious Clippy, the Level 2 profile
   guard, and the complete Rust workspace lane once at the final candidate.
6. Confirm no `native_v2.rs`, ABI header, golden, denominator, mission data, or
   pinned remc2 diff was introduced.
7. Run UBS only on changed Rust files and triage results.

No Unity run is required for a proven internal pure move with an unchanged ABI.
If the ABI changes, the refactor is no longer neutral and must stop rather than
expanding its gate set.

## Test topology

Tests beside their behavior are normal and idiomatic Rust. The problem is not
that `native_world.rs` contains tests; it is that one giant behavior hub also
contains one giant 18K-line test hub and is the reservation target for unrelated
features.

Move focused tests with the behavior they discriminate. Keep cross-domain
composition, reset/replay, hash, and production-route tests in the parent until
a proven owner emerges. Do not bulk-extract the entire test module: that adds
blame churn and does not open a production writer lane.

## Model triangulation and falsifiers

Independent Fable, Grok 4.6, and Oracle passes agreed on single authority and
rejected a broad subsystem rewrite. Their useful disagreement was whether
per-model files create enough real throughput:

- Fable: yes, because the current file reservation is a literal mutex and Rust
  privacy permits a low-risk textual move.
- Grok: only after the shared spine is explicit and frozen; NewEvent order,
  arena kinds, RNG, hash, and reset still serialize.
- Oracle: make one neutral handler extraction, then use the smallest model as
  the falsifier; do not confuse model naming with state ownership.

The deciding evidence is the post-extraction packet ledger, not model voting.
The cut loses if most implementation lines for three or more packets still
land in `native_world.rs` or `level2_class5/mod.rs`, or if the move requires a
second authority, new public interface, placeholder handler, or changed
protected observation. In that case, preserve any independently useful green
single-family extraction and serialize the remaining packets.

## Reusable learning receipts

These are candidates for later improvement of Oliver-owned skills. No JSM
skill file is changed by this work.

### Rust child privacy is a low-entropy migration edge

- Origin: the working Nyphur and dynamic-economy child modules.
- Consumer: isomorphic de-monolithization and semantic-architecture workflows.
- Mechanism: a child sees parent-private items, so an `impl` and its tests can
  move without widening visibility or duplicating state.
- Falsifier: the move requires a new public API or changes a consumer path.
- Reuse trigger: a same-crate monolith with a coherent behavior packet.
- Retirement: when the authority is deliberately split across crates.

### Co-change evidence must be age-normalized

- Origin: both current child seams are only one day old, making raw counts
  misleading.
- Consumer: seam discovery and topology optimization.
- Mechanism: compare co-change rates from file creation and require enough
  observations before claiming a causal throughput result.
- Falsifier: the normalized rate remains equal or worse after a meaningful
  sample, such as 20 seam-touching commits.
- Reuse trigger: any recently extracted module.
- Retirement: once a stable long-window sample exists.

### A file reservation can make extraction a scheduling optimization

- Origin: four ready packets on one reserved file.
- Consumer: Beads decomposition and multi-writer planning.
- Mechanism: even without a new semantic authority, moving coherent bodies to
  disjoint files can reduce the mutable boundary cut seen by workers.
- Falsifier: most edit time remains in the shared parent/spine.
- Reuse trigger: observed multi-leaf contention, not file size alone.
- Retirement: when coordination safely supports logical sub-file reservations.

### Source class or model tag is not ownership

- Origin: class-5 handlers share the arena, RNG, source cursor, hash, and reset;
  Nyphur also observes class-5 population.
- Consumer: behavioral factorization and semantic architecture.
- Mechanism: define ownership from state transitions and protected
  observations, not taxonomy labels.
- Falsifier: a candidate module can evolve independently without crossing the
  shared state or observation boundary.
- Reuse trigger: ports from tagged unions, event classes, or opcode families.
- Retirement: never; the evidence can change the particular owner, not the
  rule.

### Dispatch predicates belong with behavior only when composition-free

- Origin: class-5 target and action guards currently live in the parent
  dispatcher.
- Consumer: seam-topology refactors.
- Mechanism: a domain-only predicate left in a parent guarantees future
  contention; a cross-domain predicate is honest composition and must stay.
- Falsifier: moving it changes ordering, borrowing, or another domain's view.
- Reuse trigger: large enum/match dispatchers.
- Retirement: once the dispatcher surface is stable and no longer contended.

### Translation validation needs more than a green suite

- Origin: Fable's initial claim that unchanged test count plus green was
  sufficient was rejected in cross-examination.
- Consumer: isomorphic-refactor verification.
- Mechanism: combine token-identical move review, unchanged test bodies and
  census, focused crossing tests, public-path compilation, and forbidden-diff
  checks.
- Falsifier: a seeded branch/order mutation passes the evidence bundle.
- Reuse trigger: any behavior-neutral extraction from order-sensitive code.
- Retirement: when a stronger automatic equivalence checker subsumes it.
