# Remaining campaign census (`mc-iol7n`)

This is a source-shaped, bounded census of external Levels 3–24. Each package was produced twice by the pinned private exporter and the complete package trees matched byte-for-byte. No original executable, DOSBox session, decompiler, simulation implementation, or ABI change is part of this receipt.

- remc2 specification commit: `6806c7204d17220ecc0430759f784aea8ee2ddb2`
- levels: `3..24` (22 entries)
- byte-identical twin levels: `22`
- generated roots (ignored/private): `research/private/generated/mc-iol7n-campaign-census-run-a-r2`, `research/private/generated/mc-iol7n-campaign-census-run-b-r2`
- committed data is metadata only: package/member/source hashes, bounded tuples, objective forms, map gates, and source-fact diffs.

## Per-level inventory

| Level | Internal | Map | Active entities | Authored rows | Tuple keys | Closure keys | New tuple keys | New objective values | Source-fact deltas |
|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|
| 3 | 2 | Day | 252 | 263 | 71 | 1117 | 66 | 5 | 14 |
| 4 | 3 | Cave | 181 | 918 | 176 | 1123 | 170 | 3 | 4 |
| 5 | 4 | Day | 277 | 314 | 77 | 1065 | 69 | 6 | 0 |
| 6 | 5 | Cave | 81 | 746 | 233 | 1125 | 226 | 6 | 0 |
| 7 | 6 | Night | 370 | 499 | 170 | 1117 | 161 | 7 | 2 |
| 8 | 7 | Cave | 38 | 545 | 143 | 1122 | 138 | 4 | 0 |
| 9 | 8 | Day | 161 | 471 | 164 | 1116 | 156 | 10 | 0 |
| 10 | 9 | Night | 314 | 487 | 116 | 1120 | 111 | 4 | 4 |
| 11 | 10 | Night | 169 | 359 | 51 | 1115 | 46 | 4 | 0 |
| 12 | 11 | Cave | 112 | 836 | 228 | 1124 | 223 | 0 | 0 |
| 13 | 12 | Day | 172 | 494 | 102 | 1119 | 93 | 5 | 6 |
| 14 | 13 | Night | 625 | 595 | 120 | 1116 | 114 | 10 | 16 |
| 15 | 14 | Cave | 255 | 650 | 168 | 1122 | 163 | 9 | 4 |
| 16 | 15 | Cave | 165 | 731 | 222 | 1120 | 217 | 9 | 0 |
| 17 | 16 | Day | 388 | 454 | 110 | 1117 | 102 | 6 | 6 |
| 18 | 17 | Night | 210 | 432 | 160 | 1116 | 154 | 10 | 0 |
| 19 | 18 | Night | 552 | 780 | 135 | 1115 | 129 | 8 | 0 |
| 20 | 19 | Day | 324 | 517 | 57 | 932 | 52 | 7 | 18 |
| 21 | 20 | Cave | 255 | 918 | 256 | 1124 | 250 | 8 | 2 |
| 22 | 21 | Night | 253 | 473 | 130 | 1124 | 126 | 10 | 0 |
| 23 | 22 | Day | 617 | 718 | 156 | 1065 | 149 | 8 | 32 |
| 24 | 23 | Cave | 207 | 503 | 134 | 1124 | 128 | 3 | 4 |

## Bounded diff notes

The tuple inventory uses the source-shaped order `class/action/model/subtype`. For authored rows those fields are `type_0x30311 / DisId (0xFF represents source -1) / par1_14 / subtype_0x30311`; reset rows use the captured entity `class / action / model / xsubtype` bytes. Both domains remain visible so a field-vocabulary collision is not silently normalized.

The behavior closure is the union of enabled source action and constructor table rows for classes observed in the packages, plus authored type/subtype selectors. This is conservative and deterministic; it is not a claim that dynamic predicates, callback effects, or all transitive runtime transitions are implemented.

Map gates are reported as exact exporter identities. Current implemented profile capability remains external Level 1/internal 0/Night and external Level 2/internal 1/Day; each Level 3–24 row therefore reports a missing implemented profile even when its map type or tuple shape matches the accepted Level 2 baseline.

Required source facts include authored level records, feature-construction inputs, and collision-mask selectors. The diff distinguishes known fact kinds from newly observed selector values and owner-artifact identities. Presentation dependencies are recorded separately and are never treated as simulation authority.
