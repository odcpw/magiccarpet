# Known conformance divergences

## DISC-001: Alternative hypotheses are packet-bound, not experiment dependencies

- Reference behavior: the packet preregisters preferred and alternative
  hypotheses.
- Registry behavior: `experiment.hypothesis_ids` participates in semantic
  invalidation fan-out, so a correctly falsified alternative would force the
  completed experiment into `RETEST_REQUIRED`.
- Suite behavior: the experiment binds the preferred hypothesis; the immutable
  packet independently binds both alternatives and their exact predictions.
- Impact: no loss of preregistration, but the alternative is not a typed
  experiment dependency.
- Resolution: `ACCEPTED` pending a registry contract decision.
- Tests affected: P1-P5 hypothesis terminal-state checks.
- Review date: 2026-07-23.

## DISC-002: Contradiction resolution is committed in one atomic batch

- Reference behavior: an open contradiction would ordinarily be recorded,
  investigated, amended, then resolved.
- Registry behavior: after affected evidence becomes `CONTRADICTED`, later
  contradiction amendment is rejected because `affected_ids` points at an
  invalidating state.
- Suite behavior: P4 atomically creates the open record, amends its disposition,
  transitions it to the exact resolution, and moves dependents to safe states.
- Impact: the event history retains CREATE, AMEND, and TRANSITION, but there is
  no committed intermediate open root.
- Resolution: `ACCEPTED` for this deterministic smoke; investigate separately
  if persistent open-contradiction workflows are required.
- Tests affected: P4 atomic contradiction resolution.
- Review date: 2026-07-23.
