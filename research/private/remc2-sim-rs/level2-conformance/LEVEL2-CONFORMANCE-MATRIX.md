# Level 2 conformance matrix (generated view)

Canonical JSON: [`level2-conformance-matrix.json`](level2-conformance-matrix.json)

> This is an honest open matrix. It records the full Level 2 denominator and ownership before behavior closure; no row is Exact yet.

Pinned remc2: `6806c7204d17220ecc0430759f784aea8ee2ddb2`. Generated: `2026-08-02`.

## Status

**38 rows**: Exact=0, Missing=25, Partial=0, Unproved=13, Wrong=0; **9 mechanical exclusions**; **38 owning beads**.

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
| `ROW-mc-0qxf` | presentation | **Unproved** | `mc-0qxf` | — |
| `ROW-mc-1agi` | gameplay | **Unproved** | `mc-1agi` | — |
| `ROW-mc-1g06` | ai | **Missing** | `mc-1g06` | — |
| `ROW-mc-2eby` | ai | **Unproved** | `mc-2eby` | — |
| `ROW-mc-2sbd` | gameplay | **Missing** | `mc-2sbd` | — |
| `ROW-mc-38gs` | gameplay | **Missing** | `mc-38gs` | — |
| `ROW-mc-4ipu` | ai | **Missing** | `mc-4ipu` | dispatch |
| `ROW-mc-5iyv` | gameplay | **Unproved** | `mc-5iyv` | — |
| `ROW-mc-6dqd` | ai | **Missing** | `mc-6dqd` | — |
| `ROW-mc-94h7` | presentation | **Missing** | `mc-94h7` | — |
| `ROW-mc-a73w` | presentation | **Missing** | `mc-a73w` | — |
| `ROW-mc-a86h` | gameplay | **Missing** | `mc-a86h` | — |
| `ROW-mc-app8` | ai | **Missing** | `mc-app8` | — |
| `ROW-mc-c2by` | ai | **Missing** | `mc-c2by` | — |
| `ROW-mc-clng` | gameplay | **Missing** | `mc-clng` | — |
| `ROW-mc-e7oi` | ai | **Unproved** | `mc-e7oi` | ai_helpers |
| `ROW-mc-ewsc` | rng-audio | **Unproved** | `mc-ewsc` | cache, presentation_refs |
| `ROW-mc-exzm` | presentation | **Missing** | `mc-exzm` | reset |
| `ROW-mc-fgrh` | gameplay | **Missing** | `mc-fgrh` | — |
| `ROW-mc-fi4n` | gameplay | **Unproved** | `mc-fi4n` | — |
| `ROW-mc-hgk9` | gameplay | **Missing** | `mc-hgk9` | — |
| `ROW-mc-jc4w` | gameplay | **Unproved** | `mc-jc4w` | tuples |
| `ROW-mc-jmcp` | gameplay | **Missing** | `mc-jmcp` | — |
| `ROW-mc-k5js` | ai | **Missing** | `mc-k5js` | — |
| `ROW-mc-mmv3` | gameplay | **Missing** | `mc-mmv3` | — |
| `ROW-mc-nr19` | gameplay | **Missing** | `mc-nr19` | objectives |
| `ROW-mc-oiaa` | presentation | **Missing** | `mc-oiaa` | — |
| `ROW-mc-ptqs` | gameplay | **Missing** | `mc-ptqs` | — |
| `ROW-mc-qkt3` | ai | **Unproved** | `mc-qkt3` | — |
| `ROW-mc-sq58` | ai | **Missing** | `mc-sq58` | — |
| `ROW-mc-uk2k` | gameplay | **Missing** | `mc-uk2k` | authored, activation_groups, producer_edges |
| `ROW-mc-uptb` | ai | **Missing** | `mc-uptb` | — |
| `ROW-mc-vm2n` | gameplay | **Missing** | `mc-vm2n` | — |
| `ROW-mc-wl60` | gameplay | **Unproved** | `mc-wl60` | — |
| `ROW-mc-wv4o` | gameplay | **Unproved** | `mc-wv4o` | — |
| `ROW-mc-xcgi` | gameplay | **Unproved** | `mc-xcgi` | — |
| `ROW-mc-xm2n` | presentation | **Unproved** | `mc-xm2n` | — |
| `ROW-mc-yawx` | rng-audio | **Missing** | `mc-yawx` | rng, audio |

## Mechanical exclusions

| exclusion | scope |
|---|---|
| `EXC-L2-AUTHORED-NOCALL` | 755 deliberate-no-call authored records |
| `EXC-L2-AUTHORED-MECHANICAL` | 87 mechanically excluded authored records |
| `EXC-L2-RESET-INACTIVE` | 886 inactive reset slots |
| `EXC-L2-GROUP-30` | activation group 30 |
| `EXC-L2-DISPATCH-NONROOTED` | 787 non-rooted/disabled/null dispatch slots |
| `EXC-L2-RNG-NONROOTED` | 414 non-rooted RNG occurrences |
| `EXC-L2-AUDIO-NONROOTED` | 93 non-rooted audio calls |
| `EXC-L2-CACHE-NONROOTED` | 967 presentation cache records |
| `EXC-L2-PRESENTATION-NONROOTED` | 818 source presentation references |
