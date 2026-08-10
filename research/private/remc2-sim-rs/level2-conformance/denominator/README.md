# Level 2 dispatch/RNG/audio/presentation denominator

> Status: historical/reference-only. This denominator is not part of the
> ordinary Level 2 development gate and is not runtime behavior proof.

`level2-dispatch-rng-audio-presentation.json` is the pinned Level 2
denominator for the dispatch slots, authored/reset tuple join, AI helper
closure, global/entity RNG sites, the existing 119-call
`PrepareEventSound_6E450` ledger, and the Level 2 owner presentation cache.

The builder reads remc2 only with `git show` at
`6806c7204d17220ecc0430759f784aea8ee2ddb2`.  It never reads or modifies the
protected upstream worktree.  The authored/reset census and activation
closure are inputs, while the 119 audio rows are reused by identity rather
than extracted into a competing ledger.

The output contains:

* all 845 physical constructor/action dispatch slots, with the 58 slots
  reachable from Level 2 reset/authored tuples and explicit sentinel,
  disabled, and mechanically-unreachable partitions;
* the 373 authored/reset tuple records and the complete activation transition
  closure, including all 31 producer edges;
* the transitive `sub_12910` AI helper ledger, 14 mode entries, common-tail
  ordering/multiplicity, and pinned call edges;
* every pinned LCG/RNG source occurrence and its reachability partition;
* all 119 existing sound-call identities reclassified for Level 2; and
* 347 particle, 504 sprite-selector, and 157 entity-parameter cache rows,
  plus lexical TMAP/BUILD/sprite/HUD/text/music source references and their
  native/Unity owner declaration.

Presentation source reachability is intentionally rooted: the JSON exposes
the curated per-module `module_roots`, and each source file/reference carries
`root_binding`.  Lexical references outside those roots remain explicit
mechanical exclusions in the source inventory; they are not silently dropped.
The Day profile also records the cave RNG/audio exclusions, including their
stable exclusion IDs and proof.

Explicit archival regeneration/check from the repository root (not a
per-slice command):

```text
python3 research/private/remc2-sim-rs/level2-conformance/denominator/build_level2_denominator.py --write
python3 research/private/remc2-sim-rs/level2-conformance/denominator/build_level2_denominator.py --check
python3 -m unittest research/private/remc2-sim-rs/level2-conformance/denominator/test_level2_denominator.py
```

Every row has an explicit classification.  `unclassified` is a hard failure;
an excluded row remains in the denominator with its source identity and proof
instead of disappearing from the artifact.
