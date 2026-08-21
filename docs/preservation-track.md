# MC2 preservation and oracle track

Version: 1.0  
Status: governing private-track contract  
Track: `P`

## Outcome

Maintain a reproducible, deeply understood, testable MC2 runtime for Oliver's
owned data, based on the existing reconstruction ecosystem and checked against
the original executable.

This track preserves history and fidelity. It does not create the commercial
successor and does not imply public distribution rights.

## Canonical sources

- Original executable/data: exact hashes in the private source register.
- Reconstructed source: `turican0/remc2`, pinned per experiment.
- HD release/community lane: `thobbsinteractive/magic-carpet-2-hd`.
- Original oracle: `turican0/dosbox-x-remc2`.
- Modern renderer precedent: `turican0/MagicBalls`.
- Static/dynamic analysis: Ghidra, Binary Ninja, DOSBox, debugger and controlled
  capture tools registered per campaign.

Branch names are never sufficient identifiers. Each build, recording,
snapshot, result, or note records exact commits, source-data profile, compiler,
architecture, configuration, and artifact hash.

## Fidelity workflow

```text
owned original executable in instrumented DOSBox
   + deterministic input
   + exact instruction/checkpoint trigger
   -> registers and private state snapshots

pinned native reconstructed build
   + same input and checkpoint
   -> state comparison
   -> first divergence
   -> explanation/fix/known-difference record
```

Visual similarity is supplementary. Internal state, replay, save/load, RNG
ordering, entity state, terrain, economy, AI, and lifecycle comparisons are
the primary fidelity evidence where available.

## Modernization boundaries

Allowed private experiments include:

- build-system and portability repair;
- renderer/audio/input/filesystem adapters;
- headless and deterministic-step extraction;
- native 64-bit correctness;
- regression tooling;
- owner-local asset discovery/import;
- high-resolution and accessibility experiments;
- MagicBalls/Godot comparison;
- C ABI and Wasm spikes;
- private multiplayer observation; and
- private VR presentation experiments.

Every change states whether it is:

- fidelity correction;
- portability repair;
- presentation modernization;
- developer tooling;
- deliberate behavior deviation; or
- unresolved experiment.

## Browser proof

The browser proof is incremental:

1. compile a noninteractive support component;
2. compile deterministic simulation for a bounded checkpoint;
3. load owner-selected local data without upload;
4. advance one level segment and compare native/Wasm state;
5. drive a minimal renderer/audio/input shell;
6. measure memory, startup, filesystem, user-gesture audio, worker/thread and
   background-tab behavior;
7. decide whether a complete owner-only browser build is viable.

No public URL or friend build follows automatically.

## Upstream respect

- Preserve authorship and Git history.
- Do not relabel the reconstruction as our work.
- Prefer upstreamable portability, tests, docs, and bounded fixes where terms
  permit.
- Coordinate before presenting a preservation release as a continuation.
- Record contributions, licenses, notices, and source-offer duties.
- Consider sponsorship or other material support.

## Exit criteria

Track P has a complete first milestone when:

- stable and development native builds are reproducible;
- one deterministic original/native comparison is reproduced;
- known divergent/skipped coverage is explicit;
- private data and evidence retention is sound;
- the core/host dependency map exists;
- Wasm/headless feasibility has evidence; and
- public/private/license decisions remain explicit rather than assumed.

## Related implementation architecture

See
[`preservation-campaign-seam-geometry.md`](preservation-campaign-seam-geometry.md)
for the private Rust campaign ownership and source-scheduler hypothesis, and
[`preservation-program-folding-research.md`](preservation-program-folding-research.md)
for the constrained-hypergraph, MDL, migration-landscape, and protein-folding
research behind its post-Level-2 generalization.
