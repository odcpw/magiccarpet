# Carpet Flight private and future-public boundary

Status: **governing repository/data boundary, 2026-08-21**
Authority: [`program-charter.md`](program-charter.md)

## Decision

`/home/oliver/Projects/odcpw/magiccarpet` on `3090` is the one canonical private
repository for the current engine, MC2-owned compatibility package, Unity
client, Three.js/CLI harness work and future original packages. There is no
active sibling successor repository and no Track-P-to-Track-S transfer process.

The current repository is `PRIVATE_LAB`. A future public or commercial output
is an explicitly reviewed artifact/package subset, not an automatic
reclassification of this checkout.

## Data classes

| Class | Examples | Allowed use |
|---|---|---|
| `PRIVATE_OWNER_DATA` | Oliver's MC2 files, mounted media, decoded owner cache | Local `MC2-owned` runtime and validation only |
| `PRIVATE_COMPATIBILITY` | reconstructed/source-shaped code, exact source IDs, raw traces, compatibility maps | Private engine/adapter development and evidence |
| `PRIVATE_PROJECT` | repository source, tests, docs, Beads, local captures and generated work-in-progress | Private development; release only after explicit review |
| `RELEASE_CANDIDATE` | independently authored source/assets with complete provenance selected for one candidate | Exact bounded review; not yet approved |
| `APPROVED_RELEASE` | artifacts that passed the named release gate | Distribution under the recorded terms |

Transformation does not automatically change classification. A converted,
hashed, cropped, regenerated or re-encoded owner artifact remains private until
an explicit provenance/release decision says otherwise.

## Repository and runtime placement

- Keep owner packages, closure output, decoded caches, captures and private
  generated outputs under ignored/external paths documented by their tools.
- Never copy them into tracked Unity `Assets`, `StreamingAssets`, checked-in
  web assets, release bundles or public CI fixtures.
- Package manifests may refer to owner-local content by stable private identity;
  portable manifests and receipts must not leak absolute owner paths.
- Secrets, service tokens, `.env` values and agent authentication/configuration
  never enter Git or portable receipts.
- Synthetic fixtures must be independently authored and visibly synthetic;
  they are not evidence that owner-data routes work unless the real route is
  separately executed.

## Access and egress

The repository and owner inputs stay on Oliver-controlled private systems and
approved private services. Before external egress, apply
[`provenance-policy.md`](provenance-policy.md): name the exact payload, service,
purpose, retention and sanitization.

Do not upload owner data, raw traces, reconstructed source, private Beads,
credentials or sensitive paths to generative services, hosted build systems,
issue trackers or public paste sites without explicit approval.

## Builds

A local executable that depends on ignored owner inputs is a private runtime
artifact, not a deployed release. Release claims require the exact candidate
build, input inventory and publication target.

Before packaging or publishing anything:

1. materialize a source/package/runtime inventory by content hash;
2. prove no `PRIVATE_OWNER_DATA` or unresolved `PRIVATE_COMPATIBILITY` artifact
   is embedded;
3. scan for secrets, private paths, owner filenames and unintended telemetry;
4. verify dependency notices and content provenance;
5. run the candidate's functional and privacy gates; and
6. obtain Oliver's explicit release authorization.

No such release is currently claimed. `gh release list` returned no repository
release during the 2026-08-21 recovery assessment.

## Future open-source or commercial structure

If a public repository or distribution split later becomes useful, derive it
from a reviewed allowlist of independently authored engine/client/package
artifacts. Do not recursively copy this private repository or infer safety from
directory names. The engine architecture remains one engine regardless of
publication topology.

## Incident response

On suspected boundary breach:

- stop further distribution or egress;
- preserve the exact affected artifacts, hashes and logs;
- identify where the material came from and every destination;
- rotate exposed credentials if any;
- remove/rebuild the candidate through a reviewed, recoverable change; and
- record the final verification and remaining uncertainty.

Do not destroy source evidence, rewrite shared history or declare the incident
closed based only on a filename scan.
