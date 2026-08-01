# Level 2 activation closure

`level2-activation-closure.json` is the exhaustive Level 2 authored-row
closure for the pinned remc2 specification commit
`6806c7204d17220ecc0430759f784aea8ee2ddb2`.

The derivation starts at the production root `LevelInitGame_56A30 ->
sub_4A1E0(0, 1)`, records every producer edge found in the authored rows, and
checks the dispatch/producer/objective source anchors with `git show` from the
pinned upstream checkout.  It distinguishes four outcomes for every one of
the 1,200 authored slots:

* `reachable`: consumed directly by the root group;
* `conditionally_reachable`: reached through a producer or the sentinel
  preprocessor, but gated by gameplay/switch/building conditions;
* `deliberate_no_call`: an all-zero authored slot with no event type; and
* `mechanically_excluded`: type-zero terrain auxiliaries or groups with no
  rooted producer/consumer.

The artifact also names all 31 producer edges and the explicit model45
`par2=0` no-op row, every direct dispatcher
callsite, the terminal group-30/group-32 exclusions, the requested dormant
constructor tuples with exact `sub_4A310` branch evidence, and the five Level 2
objectives (fly to `(2,152)`, banish player, kill authored row 229, fly to
`(154,229)`, and destroy the building chain rooted at row 153).

Regenerate or validate it from the repository root:

```text
python3 research/private/remc2-sim-rs/level2-conformance/activation/build_level2_activation_closure.py --write
python3 research/private/remc2-sim-rs/level2-conformance/activation/build_level2_activation_closure.py --check
python3 -m unittest research/private/remc2-sim-rs/level2-conformance/activation/test_level2_activation_closure.py
```

The checker deliberately reads source using `git -C upstream/magic-carpet-2-hd
show <pinned-commit>:<path>` so a dirty upstream worktree cannot silently
change the conformance result.
