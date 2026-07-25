# Private evidence registry

This directory implements the append-only registry required by
`docs/behavioral-reverse-engineering-protocol.md` version 0.2. It is private
research infrastructure, not production game code and not a transfer package.

## Authority and storage

`events.jsonl` is the only authoritative record. Each canonical JSON line has:

- a sequential event ID;
- a global predecessor hash;
- an entity-local predecessor hash and version;
- a SHA-256 over every field except `event_hash`;
- a strict entity ID, lifecycle state, classification, author, timestamp and
  payload;
- a closed entity-specific `detail` manifest with its own schema ID, artifact
  ID and verified SHA-256.

Markdown and JSON views are derived. Corrections append `AMEND`, `TRANSITION`
or `LINK` events; existing logical events are never changed. The writer uses
an exclusive advisory lock, validates the complete candidate chain, stages a
complete owner-only stream, `fsync`s it, atomically replaces the authority,
and `fsync`s the directory. A failed pre-commit write leaves the prior stream
unchanged. Symlinks, non-regular registry files, and group/other access are
rejected.

The event schema is `event.schema.json`. `registry.py` enforces the schema and
the protocol rules using only the Python standard library.

## Important semantics

- A link is directed from evidence/upstream to consumer/downstream. Transfers,
  reviews and receipts cannot be versioned with `LINK` or `AMEND`.
- `supports`, `refines`, `verifies`, and `implements` participate in
  invalidation fan-out.
- Cycles are forbidden through `supports`, `refines`, `supersedes`,
  `verifies`, and `implements`.
- A falsified, superseded, revoked, contradicted, invalid, or retest-required
  upstream forces every semantic dependent—including closed ID fields and
  `references`—into an explicit blocked/revoked/retest state. An open
  contradiction directly blocks every entity in its `affected_ids`.
- A current receipt for a revoked, superseded, or retest-required transfer is
  invalid.
- A finding, vector, product deviation or transfer cannot be accepted/approved
  without a non-author review of its exact reviewable event. Reviews record
  explicit checks and context independence; inherited raw analyst context is
  rejected.
- Reviews are general exact-target records. Transfers are reviewed at
  `IN_REVIEW`; findings and vectors at `REVIEWED`.
- Accepted findings must be public-safe or synthetic, meet their declared
  evidence floor, name enough independent evidence routes, and have no C2/C3
  contradiction. Timing, determinism, network, economy and ordering domains
  automatically require the critical floor.
- Approved transfers require accepted findings plus accepted nonempty
  synthetic vectors from one campaign. Vectors contain structured positive,
  boundary, rejection and invariant cases plus a no-original-payload
  attestation. Amber cannot be laundered through a Green transfer.
- A consumed transfer requires a current exact-latest-hash receipt whose
  destination and files match the transfer. The receipt also binds every
  accepted vector event hash and named implementation artifact hashes.
- Semantic ID fields are mandatory typed dependencies even when the optional
  prose `references` list is absent. Nested IDs embed the exact declared parent.
- Open campaigns require registered `PASSED` calibrations. Hypotheses and
  findings bind registered questions and typed evidence entities; vectors bind
  accepted source findings. The detail manifest copies of these IDs must match
  the closed CREATE fields exactly.
- Every CREATE carries a closed v0.2 detail manifest covering the protocol's
  source/license/exposure/retention, launch chain, environment/calibration,
  preregistration, sampling/control, capture, measurement, mechanism,
  functional-rule, review, transfer and closeout categories. Its inline
  content and verified hash avoid treating an opaque hash as validation.
- Red vectors and Red transfers are rejected.
- Multi-entity lifecycle changes, such as staling a receipt while revoking its
  consumed transfer, use `append-batch`; the complete candidate chain is
  validated before any bytes are appended.

The registry cannot itself prove that a consuming Bead was notified. The final
lab gate must compare stale registry receipts with Beads before closure.

## CLI

Initialize a new owner-only registry:

```sh
python3 research/private/registry/registry.py init \
  --registry research/private/evidence-registry \
  --author oliver
```

Append an event spec. The spec omits sequence/version/hash-chain fields; the
writer computes them:

```sh
python3 research/private/registry/registry.py append \
  --registry research/private/evidence-registry \
  --spec /private/path/to/event-spec.json
```

Append an atomic JSON array of event specs:

```sh
python3 research/private/registry/registry.py append-batch \
  --registry research/private/evidence-registry \
  --specs /private/path/to/event-specs.json
```

Validate or render:

```sh
python3 research/private/registry/registry.py validate \
  --registry research/private/evidence-registry

python3 research/private/registry/registry.py render \
  --registry research/private/evidence-registry \
  --format markdown
```

Never place an event spec, registry, derived private view, or analyzer ID in a
future public checkout.

## Event spec shape

```json
{
  "event_type": "CREATE",
  "entity_type": "source",
  "entity_id": "SRC-SYNTH-0001",
  "author": "analyst-a",
  "classification": "SYNTHETIC_ONLY",
  "status": "REGISTERED",
  "payload": {
    "title": "Known-source synthetic fixture",
    "source_class": "SYNTHETIC",
    "artifact_hashes": {
      "fixture.bin": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    },
    "detail": {
      "schema_id": "mc2-behavioral-registry/source/0.2",
      "artifact_id": "DET-SRC-SYNTH-0001",
      "manifest": {
        "approved_use": "Synthetic registry qualification only",
        "evidence_grade": "KNOWN_SOURCE_SYNTHETIC",
        "exposure": {
          "execution": "local",
          "egress": "none"
        },
        "license": "Project-authored synthetic fixture",
        "license_risk_disposition": "GREEN",
        "modification": {
          "kind": "UNMODIFIED"
        },
        "origin_lineage": "Project-authored known-source fixture",
        "retention_disposition": {
          "purpose": "registry tests",
          "state": "retain"
        },
        "revision": "1"
      },
      "manifest_sha256": "2e0472fb8cde61318b100f1e41e509af742f288c7fb0297592f16af9b085e45b"
    }
  }
}
```

For a later event, repeat the same `entity_type`, `entity_id`,
`classification`, and current or next lifecycle `status`. A normal
`TRANSITION` payload requires `reason`; finding/vector `REVIEWED` and
`ACCEPTED` transitions also require `independence`. A `LINK` payload requires
`target_id` and `relation`. An amendable entity uses exactly `reason` plus a
nonempty `changes` object; accepted/immutable records cannot be amended in
place.

## Verification

Run:

```sh
PYTHONDONTWRITEBYTECODE=1 \
  python3 -m unittest discover \
  -s research/private/registry \
  -p 'test_*.py' \
  -v
```

The suite covers canonical and detail-manifest hashing, tamper detection,
reused and parent-mismatched IDs, typed semantic dependencies, closed payload
types, broken lineage, invalid transitions, finding/vector/transfer reviewer
independence, exact-version review and receipt binding, destination/vector hash
equality, derived critical evidence floors, private/C2/C3 rejection, Red
vectors, Amber laundering, direct-contradiction and semantic revocation
fan-out, failure-atomic staged commits, symlink rejection, owner-only storage
and exact restore.
