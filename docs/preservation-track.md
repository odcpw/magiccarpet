# MC2-owned compatibility and preservation guide

Status: **subordinate compatibility contract; no longer a separate governing
track**
Authority: [`program-charter.md`](program-charter.md)

## Outcome

Preserve and complete MC2 behavior as the first real Carpet Flight game package
while extracting one reusable engine. Private owner-local MC2 play and exact
compatibility research are legitimate project outcomes; they do not form a
second product or a one-way clean-room lesson pipeline.

## Canonical inputs

- remc2 submodule commit
  `6806c7204d17220ecc0430759f784aea8ee2ddb2` is the source implementation;
- Oliver's owned local data is the authoritative private content input;
- the remc2 level exporter, asset closure and decoded cache are the current
  production import route;
- current Rust runtime behavior and fresh verification at the candidate revision
  outrank historical documents or Bead claims; and
- original-binary/DOSBox/decompiler oracle work requires an explicit Oliver
  request. The normal route uses remc2 source and owned-data packages.

## Compatibility laws

- Preserve fixed-width/wrapping arithmetic, source-relevant slot and pass order,
  RNG draw order, event/audio order, reset isolation and deterministic hashes.
- A behavior identity includes ordinary producer/constructor, scheduler and
  dispatch key, handler, same-pass epilogue/hide, following-pass
  cleanup/reclaim, authoritative carriers and downstream consumer.
- External MC2 levels are one-based and remc2 indexes are zero-based; conversion
  occurs exactly once at the import boundary.
- Unknown profiles, assets, capabilities and source mappings fail closed.
- Owner data, derived caches, captures and exact private artifacts remain
  outside Git and distributable builds.
- A helper, direct setter, fixture-only entity, internal adapter call or
  deterministic self-agreement does not prove the real production route.

## Relationship to the neutral engine

Source IDs and exact MC2 layouts may remain inside the `MC2-owned`
compatibility adapter. Public engine/package/client APIs use stable semantic
IDs and neutral definitions.

The existing Level 1 implementation should be sealed behind a Rust domain
`GameAdapter`/`GameSession` contract rather than rewritten first. Existing
Level 1/2 native and Unity APIs are grandfathered compatibility surfaces with an
explicit inventory and retirement disposition; they may not justify new
Level-N APIs.

Every later MC2 feature slice must either:

- add validated package data through the neutral schema; or
- add one reusable compiled Rust capability with a stable semantic identity.

Level-specific source behavior may remain a package binding only when the
pinned implementation proves it is truly content-specific.

## Level 1 conformance corpus

The foundation requires a versioned, non-no-op Level 1 corpus manifest binding:

- exact owner package/cache and source revision identities;
- exact engine/plugin build identity;
- ordinary input command traces and seed;
- accepted state/conformance hashes;
- selected snapshots, ordered events and audio; and
- provenance, invocation, result and named limitations.

Run it through a built external native consumer of the neutral domain route and
through the sealed legacy route in the same build. Fixed accepted observations
prevent both routes from drifting together. The existing internal facade and
Unity determinism tests are useful seeds, but their two-run self-comparison is
not the fixed corpus.

## Campaign completion

Level 1 is the foundation fixture. Level 2 work already contains valuable
source, carrier, dispatch and transition evidence, but broad Level 2 completion
resumes only after the package/API/client foundation and custom-package proof.
Levels 2 onward then migrate through the same engine without reopening sealed
Level 1 facts unnecessarily.

“Whole MC2 complete” requires one shared campaign engine with all required
terrain/flight, entity scheduling, RNG, combat, economy, spells, objectives,
audio/effects, reset/replay/hash, save and client behavior reachable through
real owner inputs. It is not established by a sequence of isolated level demos.

## Upstream and release posture

Treat remc2 and other upstream projects respectfully: preserve attribution,
licenses and commit identities; do not claim reconstructed work as original
upstream authorship. Private compatibility does not imply public distribution
approval. A future release uses the separate artifact gate in
[`provenance-policy.md`](provenance-policy.md) without changing the one-engine
architecture.
