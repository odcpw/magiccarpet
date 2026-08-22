# Carpet Flight provenance policy

Status: **governing private-development provenance policy; not legal advice or
public-release approval**
Authority: [`program-charter.md`](program-charter.md)

## Purpose

Preserve enough provenance to reproduce private MC2 compatibility work, keep
owned and generated assets controlled, and make a later commercial release
review possible without forcing a second engine or an artificial internal
clean-room process.

## Current posture

The entire `magiccarpet` repository is a private development lab. It contains
or references:

- a pinned remc2 source implementation;
- reconstructed/source-shaped compatibility code and behavior identities;
- owner-local MC2 data and derived packages/caches outside Git;
- private captures, traces and conformance evidence;
- independently authored engine, tooling and future custom content; and
- historical plans and private Beads.

These are valid inputs to private owner-local play and engine development. They
are not automatically suitable for public or commercial distribution.

## Provenance record

Every package source or generated runtime artifact intended to persist records:

- stable source-artifact ID and content hash;
- origin class: owned reference, pinned open source, independently authored,
  commissioned, generated, third-party licensed, or unknown;
- creator/tool/provider/model and relevant version;
- prompt/recipe/configuration when available and appropriate;
- transformation tool versions and canonical parameters;
- runtime artifact hash and semantic slot binding;
- rights/license/terms evidence or an explicit unresolved disposition;
- privacy/secret/owner-data classification; and
- allowed contexts: private lab, internal test, candidate distributable, or
  approved release.

Unknown provenance fails publication into a release-candidate graph. It may be
quarantined for private investigation if clearly labeled and kept out of
portable outputs.

## MC2-owned compatibility material

Owner files, exported packages, decoded caches, captures, source numeric IDs,
raw traces and exact source-shaped mappings remain `PRIVATE_OWNER_DATA` or
`PRIVATE_COMPATIBILITY`. They may be used by the `MC2-owned` package adapter and
local conformance runs.

They must not be committed, embedded in Unity `Assets`/`StreamingAssets`, copied
into builds intended for distribution, uploaded to public services, or
reclassified merely because they were transformed. A hash, palette conversion,
mesh conversion or AI-assisted redraw does not by itself establish release
rights.

## Independently authored and generated material

Original code, art, models, animation, audio and world data may be candidates
for a later release when their records are complete. Generated output is
treated as a source artifact, not as self-proving originality or quality.

For model-assisted assets:

- record the provider/model/tool and input/output artifact hashes;
- avoid sending owner data, private source, credentials or sensitive paths to
  a service without explicit approval;
- preserve the prompt/recipe when terms and privacy permit;
- validate style, semantic-slot, geometry/animation/audio and runtime budgets;
- keep source/reference provenance distinct from the generated result; and
- require human review for branding, close imitation, trademark, recognizable
  third-party characters and other release-risk surfaces.

The model invocation need not be byte-deterministic. Deterministic validation
and transformation begin at the captured output artifact.

## Open-source dependencies

For every dependency considered for a distributable build, record version,
source, license, notices, linkage/distribution form, transitive obligations and
the exact build consumer. A dependency accepted for private research is not
automatically approved for release.

Do not remove notices, misstate authorship or publish remc2/other upstream work
under an incompatible claim. Upstream patches and attribution are separate
decisions from the private product architecture.

## Services and egress

Before sending a private artifact to an external model, storage, build, map,
telemetry or collaboration service, identify:

- exact files/fields leaving the machine;
- service and account;
- purpose and retention;
- whether the input contains owner data, reconstructed expression, credentials,
  personal data or private paths; and
- the approved sanitized alternative.

Default-deny owner data, raw traces, repository secrets and private source.
Local tools are preferred when service egress is not necessary.

## Release-candidate gate

A future public/commercial candidate requires an explicit bounded review of:

1. package and runtime artifact provenance closure;
2. replacement/removal of private owner inputs;
3. dependency licenses, notices and redistribution terms;
4. product name, trademarks and store/platform rules;
5. telemetry, privacy, network and service behavior;
6. secrets and path leakage;
7. build reproducibility and source-to-artifact inventory; and
8. legal advice where Oliver determines it is warranted.

That gate applies to the exact candidate artifact. It does not block private
engine work and it does not require a second codebase.

## Incident rule

If private or unresolved material enters a candidate output, stop publication,
preserve evidence, identify the exact affected artifacts and consumers, remove
or replace through a reviewed change, invalidate caches/builds as needed, and
record the corrective verification. Do not rewrite history or delete evidence
to make the incident disappear.
