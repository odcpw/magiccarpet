# Level 2 conformance matrix (generated view)

Canonical JSON: [`level2-conformance-matrix.json`](level2-conformance-matrix.json)

> This is an honest open matrix. It records the full Level 2 denominator and ownership before behavior closure; no row is Exact yet.

Pinned remc2: `6806c7204d17220ecc0430759f784aea8ee2ddb2`. Generated: `2026-08-04`.

## Status

**45 rows**: Exact=0, Missing=45, Partial=0, Unproved=0, Wrong=0; **9 mechanical exclusions**; **45 owning beads**.

## Denominator reconciliation

| family | count | covered by rows/exclusions |
|---|---:|---:|
| `authored` | 1200 | PASS |
| `reset` | 1000 | PASS |
| `activation_groups` | 33 | PASS |
| `producer_edges` | 31 | PASS |
| `objectives` | 5 | PASS |
| `dispatch` | 845 | PASS |
| `tuples` | 373 | PASS |
| `ai_helpers` | 79 | PASS |
| `rng` | 494 | PASS |
| `audio` | 119 | PASS |
| `cache` | 1008 | PASS |
| `presentation_refs` | 1361 | PASS |

## Rows

| row | subsystem | classification | owner | covered families |
|---|---|---|---|---|
| `ROW-mc-1g06.1` | ai | **Missing** | `mc-1g06.1` | ai_helpers, presentation_refs |
| `ROW-mc-1g06.2` | ai | **Missing** | `mc-1g06.2` | ai_helpers, presentation_refs |
| `ROW-mc-2eby.1` | presentation | **Missing** | `mc-2eby.1` | presentation_refs |
| `ROW-mc-2sbd.1` | gameplay | **Missing** | `mc-2sbd.1` | reset |
| `ROW-mc-2sbd.2` | gameplay | **Missing** | `mc-2sbd.2` | reset, dispatch, tuples |
| `ROW-mc-38gs.1` | gameplay | **Missing** | `mc-38gs.1` | authored, dispatch, tuples |
| `ROW-mc-38gs.2` | gameplay | **Missing** | `mc-38gs.2` | authored, dispatch, tuples |
| `ROW-mc-38gs.3` | gameplay | **Missing** | `mc-38gs.3` | authored, dispatch, tuples |
| `ROW-mc-38gs.4` | gameplay | **Missing** | `mc-38gs.4` | authored, dispatch, tuples |
| `ROW-mc-4ipu.1` | gameplay | **Missing** | `mc-4ipu.1` | dispatch, ai_helpers |
| `ROW-mc-4ipu.2` | ai | **Missing** | `mc-4ipu.2` | ai_helpers |
| `ROW-mc-4ipu.3` | ai | **Missing** | `mc-4ipu.3` | ai_helpers |
| `ROW-mc-50i6` | gameplay | **Missing** | `mc-50i6` | reset, dispatch, tuples |
| `ROW-mc-6dqd.1` | ai | **Missing** | `mc-6dqd.1` | ai_helpers, presentation_refs |
| `ROW-mc-6dqd.2` | ai | **Missing** | `mc-6dqd.2` | ai_helpers |
| `ROW-mc-94h7.2` | presentation | **Missing** | `mc-94h7.2` | presentation_refs |
| `ROW-mc-a73w.1` | presentation | **Missing** | `mc-a73w.1` | presentation_refs |
| `ROW-mc-a73w.2` | presentation | **Missing** | `mc-a73w.2` | presentation_refs |
| `ROW-mc-a73w.3` | presentation | **Missing** | `mc-a73w.3` | presentation_refs |
| `ROW-mc-app8.1` | ai | **Missing** | `mc-app8.1` | ai_helpers, presentation_refs |
| `ROW-mc-app8.2` | ai | **Missing** | `mc-app8.2` | ai_helpers |
| `ROW-mc-bvihb.1` | gameplay | **Missing** | `mc-bvihb.1` | reset, dispatch, tuples |
| `ROW-mc-c2by.1` | presentation | **Missing** | `mc-c2by.1` | presentation_refs |
| `ROW-mc-c2by.2` | presentation | **Missing** | `mc-c2by.2` | presentation_refs |
| `ROW-mc-clng` | gameplay | **Missing** | `mc-clng` | reset, dispatch, tuples |
| `ROW-mc-exzm.1` | presentation | **Missing** | `mc-exzm.1` | presentation_refs |
| `ROW-mc-hgk9.2` | gameplay | **Missing** | `mc-hgk9.2` | dispatch |
| `ROW-mc-jmcp.1` | gameplay | **Missing** | `mc-jmcp.1` | dispatch |
| `ROW-mc-jmcp.2` | gameplay | **Missing** | `mc-jmcp.2` | dispatch |
| `ROW-mc-k5js.1` | ai | **Missing** | `mc-k5js.1` | ai_helpers |
| `ROW-mc-k5js.2` | ai | **Missing** | `mc-k5js.2` | ai_helpers |
| `ROW-mc-mmv3` | gameplay | **Missing** | `mc-mmv3` | dispatch |
| `ROW-mc-nr19.1` | gameplay | **Missing** | `mc-nr19.1` | objectives |
| `ROW-mc-oiaa.1` | gameplay | **Missing** | `mc-oiaa.1` | reset, dispatch, tuples, cache, presentation_refs |
| `ROW-mc-oiaa.2` | gameplay | **Missing** | `mc-oiaa.2` | dispatch, cache, presentation_refs |
| `ROW-mc-oiaa.3` | gameplay | **Missing** | `mc-oiaa.3` | authored, reset, dispatch, tuples, presentation_refs |
| `ROW-mc-uk2k.1` | gameplay | **Missing** | `mc-uk2k.1` | authored, activation_groups, tuples |
| `ROW-mc-uk2k.2` | gameplay | **Missing** | `mc-uk2k.2` | authored, activation_groups, tuples |
| `ROW-mc-uk2k.3` | gameplay | **Missing** | `mc-uk2k.3` | authored, activation_groups, tuples |
| `ROW-mc-uk2k.4` | gameplay | **Missing** | `mc-uk2k.4` | authored, activation_groups, producer_edges, tuples |
| `ROW-mc-uptb.1` | ai | **Missing** | `mc-uptb.1` | ai_helpers |
| `ROW-mc-uptb.2` | ai | **Missing** | `mc-uptb.2` | ai_helpers |
| `ROW-mc-xcgi.1.1.1` | gameplay | **Missing** | `mc-xcgi.1.1.1` | reset, tuples, presentation_refs |
| `ROW-mc-yawx.1` | rng-audio | **Missing** | `mc-yawx.1` | rng |
| `ROW-mc-yawx.2` | rng-audio | **Missing** | `mc-yawx.2` | audio |

## Mechanical exclusions

| exclusion | scope |
|---|---|
| `EXC-L2-AUTHORED-NOCALL` | 755 deliberate-no-call authored records |
| `EXC-L2-AUTHORED-MECHANICAL` | 87 mechanically excluded authored records |
| `EXC-L2-RESET-INACTIVE` | 886 inactive reset slots |
| `EXC-L2-GROUP-30` | activation group 30 |
| `EXC-L2-DISPATCH-NONROOTED` | 760 non-rooted/disabled/null dispatch slots |
| `EXC-L2-RNG-NONROOTED` | 414 non-rooted RNG occurrences |
| `EXC-L2-AUDIO-NONROOTED` | 93 non-rooted audio calls |
| `EXC-L2-CACHE-NONROOTED` | 967 presentation cache records |
| `EXC-L2-PRESENTATION-NONROOTED` | 818 source presentation references |
