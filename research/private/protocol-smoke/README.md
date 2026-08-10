# Private P1-P5 protocol smoke suite

This directory implements bead `mc-51m.5` with project-authored known-source
synthetic fixtures only. It exercises the behavioral-research registry without
reading, describing, or inferring anything about Magic Carpet 2.

This is a touch-triggered auxiliary lane. It is not part of the default
remc2/Rust/Unity development command; run it when the protocol-smoke surface or
its declared fixtures change.

## One-command fresh reproduction

From `/home/oliver/Projects/odcpw/magiccarpet`, with the output path absent:

```sh
PYTHONDONTWRITEBYTECODE=1 \
  /usr/bin/python3.14 \
  research/private/protocol-smoke/runner.py \
  --output-dir /tmp/mc-protocol-smoke-fresh-worker
```

The runner verifies the pinned input manifest, creates five isolated owner-only
registries, executes each exact fixture case once, validates every registry,
checks the full sanitized result against `expected.json`, and prints one JSON
summary. A mismatch exits 2.

The output must be a new path under `/tmp` or this private package. Raw
`events.jsonl` files never leave that private/temporary output. Remove only the
exact caller-supplied directory after recording the sanitized result.

## Contents

- `packets/P1.json` through `P5.json`: normative preregistrations.
- matching `.md` files: human-readable packet views.
- `packets/fresh-worker.json`: umbrella replay contract and exact command.
- `fixtures/`: original known-source synthetic programs and provenance.
- `protocol_smoke.py`: deterministic scenario and registry harness.
- `runner.py`: one-command CLI.
- `expected.json`: critical fixture/interpreter locks plus semantic phase
  counts, status maps, and negative error classes. Runtime roots and
  byte-preservation hashes remain emitted and are checked for repeatability by
  the focused tests rather than frozen to incidental environment metadata.
- `test_protocol_smoke.py`: focused conformance and repeatability tests.
- `COVERAGE.md`: bead-requirement coverage accounting.
- `DISCREPANCIES.md`: explicit registry-contract accommodations.

## Scenario summary

- P1: modulo-100 versus saturation; deliberate `INVALID_CONFIRMATORY` run and
  dependent observation/measurement revocation.
- P2: fixed 20 ms elapsed-time accumulator versus callback count; accepted
  `CRITICAL` finding and rejected `SIMPLE_BOUNDED` candidate.
- P3: decorator/function AST plus JSON table versus runtime tags; supported
  actual mechanism, rejected false mechanism, hash and parent-ID negatives.
- P4: fail-closed open contradiction and supersession, atomic invalidation,
  exact resolved-contradiction review, C1 replacement acceptance.
- P5: contaminated material rejection, self/inherited review rejection, clean
  exact transfer approval, receipt consumption, fail-closed non-atomic
  revocation, and atomic receipt/transfer revocation.

Every expected rejection records the pre-attempt root and SHA-256 of
`events.jsonl`, then asserts both remain byte-identical.

## Verification

Focused suite:

```sh
PYTHONDONTWRITEBYTECODE=1 \
  /usr/bin/python3.14 -m unittest discover \
  -s research/private/protocol-smoke \
  -p 'test_*.py' \
  -v
```

Registry regression suite:

```sh
PYTHONDONTWRITEBYTECODE=1 \
  /usr/bin/python3.14 -m unittest discover \
  -s research/private/registry \
  -p 'test_*.py' \
  -v
```

No third-party package, network service, remote model, emulator, Ghidra
project, Beads write, or owned-game artifact is used.
