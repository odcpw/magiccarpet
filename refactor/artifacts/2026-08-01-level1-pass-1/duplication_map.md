# Level 1 cleanup opportunity map

Scored as `(estimated LOC saved * confidence) / risk`. The acceptance threshold is 2.0.

| ID | Candidate | Type / lever | Est. LOC saved | Confidence | Risk | Score | Decision |
|---|---|---|---:|---:|---:|---:|---|
| L1-ISO-01 | Move active callers from obsolete `dynamic_resident_behavior_gap_mask` to canonical `remaining_resident_behavior_mask`, retaining a deprecated compatibility alias | Type I / canonicalize call sites | 0 | 1.0 | 1 | n/a | Accept as API hygiene, not LOC reduction |
| L1-ISO-02 | Collapse four duplicated distance-loop event arms after preserving the sound-5/sound-31 census and test-only site identity | Type II / parameterize | 18 | 0.95 | 2 | 8.55 | Reject after prototype: compiler-bound audio-site census became opaque |
| L1-ISO-03 | Merge validation-only no-op overlay command arms | Type I / merge patterns | 0 after rustfmt | 1.0 | 1 | 0.0 | Reject after prototype: no net reduction |
| L1-REJ-01 | Merge equal `HouseParameter` table values | Type V rhyme | 1 | 0.7 | 3 | 0.23 | Reject: harms source-table auditability |
| L1-REJ-02 | Merge equal audio-contract arms for exit phases 6 and 8 | Type V rhyme | 7 | 0.8 | 5 | 1.12 | Reject: distinct pinned source sites are deliberate evidence |
| L1-REJ-03 | Split the 45k-line mission module mechanically during closure | File split | 0 | 0.9 | 5 | 0.0 | Defer: worthwhile later, but no behavioral/LOC yield and excessive closure diff |
| L1-REJ-04 | Clear all pedantic Clippy diagnostics | Mixed rewrite | unknown | 0.4 | 10 | — | Reject: exact-port idioms and actionable findings must be separated first |
| L1-REJ-05 | Delete the obsolete public resident-gap alias | API deletion | 5 | 1.0 | 4 | 1.25 | Reject: private crate today, but deletion is still an avoidable Rust API break |

## Do not collapse

- Separate match arms that bind different remc2 source sites, even when their current values are equal.
- Exact-width casts and wrapping arithmetic without a field-by-field proof.
- Source-slot scheduling, allocation, RNG-draw, audio-census, or presentation-layer order.
- Test modules or production families through a bulk mechanical move during the final closure pass.
