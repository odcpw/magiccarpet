# Pinned `str_D4C48ar` campaign registry probe

This directory contains a read-only measurement of the pinned remc2 class
table `str_D4C48ar` in `remc2/engine/EventsFunctions.cpp`. The extractor reads
the exact object `6806c7204d17220ecc0430759f784aea8ee2ddb2` with `git show`; it
does not read or modify the mutable remc2 worktree. The dispatch switches in
`remc2/engine/Events.cpp` provide best-effort callback identities for stored
addresses. Callback identity is provenance, not a claim of semantic
equivalence.

## Outputs

- `str-d4c48ar-inventory.json` is the complete deterministic inventory. It
  contains all 17 class-table rows (runtime classes 0 through 15 plus the
  trailing sentinel), every action and constructor slot, source lines, stored
  addresses, enabled/sentinel state, resolution status, and callback identity.
- `rust_behavior_registry.json` is a small hand-authored registry of explicit
  Level 1/2 Rust behavior keys. Entries may group a family, but every
  class/action key and level scope is written explicitly. This is a measurement
  cross-check, not a production dispatch registry.
- `rust-diff.json` compares that registry with the active pinned source action
  key universe. The source projection includes non-sentinel, enabled action
  rows with a stored address. Disabled/null rows remain visible in the full
  inventory but are not claimed as active source behavior keys.

The current pinned measurements are:

| Measurement | Count |
| --- | ---: |
| class rows including sentinel | 17 |
| action rows including sentinels | 561 |
| non-sentinel action rows | 546 |
| constructor rows including sentinels | 284 |
| non-sentinel constructor rows | 269 |
| reconstructed action callback cases | 798 |
| reconstructed constructor callback cases | 236 |
| unique active source class/action keys | 486 |
| matching explicit Rust keys | 127 |
| Rust keys absent from source | 0 |
| source keys not yet in the small registry | 359 |

The 17 class rows have these declared action/constructor array sizes; the
array sizes include each array's trailing null sentinel:

```text
class  action  constructor
    0       -           -
    1       1          10
    2      23          10
    3      14          13
    4       1           1
    5     236          30
    6       1           1
    7       1           1
    8       1           1
    9      33          32
   10     100          93
   11      46          46
   12       1           1
   13      11          11
   14      12           7
   15      80          27
   16       -           -  sentinel
```

## Commands

From the repository root:

```bash
python3 research/private/remc2-atlas/campaign_registry_probe/extract_registry.py \
  --output research/private/remc2-atlas/campaign_registry_probe/str-d4c48ar-inventory.json
python3 research/private/remc2-atlas/campaign_registry_probe/compare_registry.py \
  --output research/private/remc2-atlas/campaign_registry_probe/rust-diff.json
python3 -m unittest discover \
  -s research/private/remc2-atlas/campaign_registry_probe \
  -p 'test_*.py' -v
```

The extractor and comparer emit canonical JSON (`sort_keys=True`, UTF-8,
newline terminated). Running the extractor twice at the same pinned commit is
expected to produce byte-identical output. The focused tests also verify the
row-count invariants, declaration line, sentinel, selected callback identities,
and checked-in artifact freshness.

## Recommendation

Keep the Rust behavior registry hand-authored and source-shaped, with this
probe as a generated read-only cross-check. The registry records the Rust path,
level scope, and review evidence that a source callback table cannot establish;
the extractor supplies exact pinned provenance and catches stale or invented
implemented keys. Use `implemented_missing_from_source` as the hard drift gate:
it must remain zero. Treat `source_not_yet_implemented` as an explicit campaign
backlog and review it when selecting the next slice; its count is not a claim
that every source row is semantically required by the current playable route.

The probe can be retired when an accepted production source-keyed registry
provides equivalent pinned provenance and an independently maintained drift
gate. Until then, it is the bounded gate for changes to the campaign behavior
registry and does not add production dispatch, handler, or fold authority.
