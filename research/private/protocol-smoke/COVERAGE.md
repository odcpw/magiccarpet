# mc-51m.5 conformance coverage

| Requirement | Level | Phase | Positive | Negative | Status |
|---|---|---|---|---|---|
| Discrete-rule inference | MUST | P1 | modulo hypothesis supported | saturation falsified | PASS |
| Protocol-deviation invalidation | MUST | P1 | invalid run plus revoked evidence | incomplete invalidation rejected | PASS |
| Competing timing laws | MUST | P2 | fixed 20 ms law supported | callback-count law falsified | PASS |
| Critical evidence floor | MUST | P2 | CRITICAL finding accepted | SIMPLE_BOUNDED rejected | PASS |
| Static/dynamic correlation | MUST | P3 | AST/table/runtime agree | wrong hash and parent rejected | PASS |
| Mechanism discrimination | MUST | P3 | real mechanism supported | false mechanism rejected | PASS |
| Contradiction fan-out | MUST | P4 | atomic safe-state batch | standalone open contradiction rejected | PASS |
| Supersession | MUST | P4 | replacement accepted/original superseded | non-atomic supersession rejected | PASS |
| C1 exact review | MUST | P4 | resolved contradiction reviewed | premature acceptance rejected | PASS |
| Contaminated transfer | MUST | P5 | clean vector/transfer path | contaminated vector/transfer rejected | PASS |
| Reviewer independence | MUST | P5 | fresh exact review recorded | self/inherited review rejected | PASS |
| Receipt revocation | MUST | P5 | atomic stale/revoke batch | non-atomic revocation rejected | PASS |
| Fresh-worker reproducibility | MUST | P1-P5 | exact locked summary | hash/state mismatch exits 2 | PASS |
| Owner-only raw evidence | MUST | P1-P5 | modes verified | output outside private/temp rejected | PASS |

MUST coverage: 14/14 tested, 14/14 passing, 0 unknown, 100%.
