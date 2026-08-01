# Level 2 profile contract

This is the first Level 2 implementation seam. It freezes the selection
contract before any Level 2 behavior is ported. The machine-readable contract
is [`level2-profile.json`](level2-profile.json); the fast guard is
[`../tools/verify_level2_profile.py`](../tools/verify_level2_profile.py).

## Profile

| Field | Required value |
| --- | --- |
| External level | `2` |
| Internal level index | `1` |
| Map type | `Day` (`MapType_t::Day`, serialized value `0`) |
| Campaign | Offline |
| Players | Local player `0`, AI opponent `1`, two total, no network multiplayer |
| Owner data | Required English data |
| Desktop | `320x200`, screen-size `0`, non-stereo |

The source pin is remc2 commit
`6806c7204d17220ecc0430759f784aea8ee2ddb2`. Source archaeology must use
`git show <pin>:<path>` because `upstream/magic-carpet-2-hd` is a protected,
possibly dirty checkout. The profile anchors the Day enum in
`remc2/engine/MapType.h`, level selection in `remc2/engine/Level.cpp`, and the
fixed 320x200 VGA path in `remc2/engine/EventsFunctions.cpp`. A separate
renderer test row records a 320x240 checkpoint for external level 2; it is not
the shared fixed desktop contract and remains a later matrix concern.

## What this includes

This contract makes the external-to-internal selector and runtime profile
explicit. It is a prerequisite for Level 2 matrix rows and means that a
loader, exporter, C ABI, or Unity session cannot silently infer a different
level, map type, player ordering, language, or desktop mode from host state.

It also freezes Level 1 as a prerequisite: the accepted matrix identity is
182/182 Exact with 29 structured exclusions, with the accepted closure,
qualification policy, and Level 1 asset-cache identity all bound by hash.

## What this does not claim

This contract does **not** claim that Level 2 gameplay, entities, AI, spells,
objectives, audio, assets, or presentation are implemented. Those are later
matrix rows. Night and Cave maps, other levels or internal indices, network or
remote players, non-English/missing owner data, alternate resolutions or
stereo, active-DAC colour, widescreen presentation, and a modern overlay are
explicitly outside this profile.

The long Unity/PLAN29 qualification remains a milestone gate. The guard does
not rerun it and does not rewrite any accepted receipt.

## Fast guard

From this Rust workspace:

```bash
python3 tools/verify_level2_profile.py --json
```

The guard has two deliberately separate sections in its JSON result:

* `historical_receipts` checks immutable hashes and declared identities for the
  Level 1 matrix, owner-package manifest, asset closure, PLAN29 artifacts,
  cache seal, and native ABI header/event-code witness. It never regenerates
  any of those receipts.
* `current_checks` reruns the cheap matrix/render checks, the production-path
  Rust reset/loader tests, and the native ABI layout/header/presentation tests.
  These checks are regenerable evidence and are not
  promoted to historical receipts.

For a metadata-only check (useful while reviewing a mutation-negative test):

```bash
python3 tools/verify_level2_profile.py --no-current-tests --json
```

Every shared Level 2 change runs the full command. The long Unity/PLAN29 gate
belongs only to the milestone bead that accepts the Level 2 matrix; it is not a
per-slice repetition.

## Broadening rule

To add a Level 2 claim, create a reviewed matrix/profile revision with source
evidence at the pinned remc2 commit, add discriminating production-path tests,
and add a new owner-data/presentation receipt. Never regenerate or overwrite a
sealed Level 1 artifact in place.
