# Fresh-worker P1-P5 reproduction packet

- Packet: `PKT-SYNTH-P1-P5-FRESH-0001`, schema 1
- Context: no inherited results or campaign discussion.
- Manifest authority: `../expected.json` key `artifact_sha256`.
- Exact command:

  `PYTHONDONTWRITEBYTECODE=1 /usr/bin/python3.14 research/private/protocol-smoke/runner.py --output-dir /tmp/mc-protocol-smoke-fresh-worker`

- Precondition: the exact output path does not exist.
- Success: all input hashes, phase roots, event/entity counts, complete terminal
  status maps, expected error classes, and byte/root preservation records
  exactly match the lock.
- Generated raw event streams stay in the caller-supplied owner-only temporary
  directory. Only the sanitized summary may be transcribed.
- Denied: owned MC2 paths/bytes, network, remote models, Ghidra, Beads
  mutation, and writes outside the exact output.
- No retry. This is synthetic protocol qualification only and makes no MC2
  inference.

`fresh-worker.json` is normative.
