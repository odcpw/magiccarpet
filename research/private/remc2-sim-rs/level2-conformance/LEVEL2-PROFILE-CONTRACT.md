# Level 2 profile contract

This is the first Level 2 implementation seam. It freezes the selection
contract before any Level 2 behavior is ported. The machine-readable contract
is [`level2-profile.json`](level2-profile.json); the active guard is
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

The accepted Level 1 native header at
`crates/sim-capi/include/mc_sim.h` is sealed byte-for-byte by the historical
header hash. Additive Level 2 campaign declarations, including economy
sidecars, live in `crates/sim-capi/include/mc_sim_level2.h`; callers that use
those declarations include the additive header explicitly.

## What this includes

This contract makes the external-to-internal selector and runtime profile
explicit. It is the input contract for the Level 2 selection seam and means
that a loader, exporter, C ABI, or Unity session cannot silently infer a
different level, map type, player ordering, language, or desktop mode from
host state. The focused Rust selector test independently proves that the
accepted tuple selects Level 2 and that invalid metadata does not fall back to
Level 1.

The census, activation, denominator, and generated matrix files under this
directory remain historical/reference artifacts. They explain prior research
coverage but are not ordinary development proof and are not read by the
active profile guard.

## What this does not claim

This contract does **not** claim that Level 2 gameplay, entities, AI, spells,
objectives, audio, assets, or presentation are implemented. Those are later
matrix rows. Night and Cave maps, other levels or internal indices, network or
remote players, non-English/missing owner data, alternate resolutions or
stereo, active-DAC colour, widescreen presentation, and a modern overlay are
explicitly outside this profile.

The long Unity/PLAN29 qualification remains a milestone gate. The guard does
not rerun it and does not rewrite any accepted receipt.

## Active development guard

From this Rust workspace:

```bash
python3 tools/verify_level2_profile.py --json
```

The guard validates only the runtime profile schema/values and runs the
focused direct Rust selector test. It does not hash frozen Level 1 receipts,
load generated matrices or ledgers, read `.beads/issues.jsonl`, or rerun
unrelated Level 1 checks. The direct Rust test is independently responsible
for rejecting a wrong level/map tuple and fallback behavior.

For a profile-only mutation-negative check (without running Cargo):

```bash
python3 tools/verify_level2_profile.py --no-runtime-tests --json
```

The historical matrix/census/activation/denominator suites and Level 1
receipt checks have explicit archival/milestone uses only. They are not part
of this per-slice command and are not evidence that the Level 2 mission is
implemented.

## Broadening rule

To add a Level 2 claim, create a reviewed profile revision with source
evidence at the pinned remc2 commit and add discriminating production-path
tests. Historical matrix/reference artifacts may document the research
context, but they do not become per-slice runtime proof. Never regenerate or
overwrite a sealed Level 1 artifact in place.
