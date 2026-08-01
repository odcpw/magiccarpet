# Changelog

This changelog records private preservation milestones, not public releases.

## 2026-08-01 — Bounded Level 1 conformance closed

- Closed the remc2-derived matrix at 182/182 Exact with zero non-Exact rows
  and 29 mechanically proved exclusions.
- Accepted one clean PLAN29 Run A covering 481 Rust tests, Unity EditMode
  31/31, PlayMode 1/1, the complete mission loop, presentation, privacy,
  provenance, victory, failure, restart, and replay.
- Preserved the fixed single-player Night profile and every explicit nonclaim;
  this milestone does not claim full-game or multiplayer parity.
- Recorded accepted and rejected cleanup candidates in the
  [isomorphic cleanup ledger](refactor/artifacts/2026-08-01-level1-pass-1/LEDGER.md),
  retaining source-shaped code whenever a compact rewrite obscured proof.
- Landed by [`c6d6dc8`](https://github.com/odcpw/magiccarpet/commit/c6d6dc84165207c571f74a4c370364b5ec4bc37f), followed by a behavior-preserving
  cleanup and documentation reconciliation.
