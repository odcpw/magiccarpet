# Bounded research worker packet

This is the research-lane specialization of the future project-wide
low-context packet owned by `mc-pyin`. Command-catalog integration is owned by
`mc-3sy6`; until then every command is written in full here.

## Identity and outcome

- Packet ID / schema version:
- Packet content SHA-256: record in the dispatch/review manifest, not inside
  the hashed packet itself
- Campaign / question / experiment IDs and versions:
- One-sentence outcome:
- Downstream decision:
- Requester / assigned worker:
- Independent reviewer:
- Prior context/exposure and conflict declaration:

## Immutable inputs and ownership

- Exact cwd:
- Source / BuildManifest / Environment / Calibration / tool IDs:
- Immutable input paths and SHA-256:
- Allowed read paths:
- Denied paths, tools, services and network:
- Exclusivity lock or file/project owner:
- Worker-owned output paths:
- Files the worker must not modify:

## Preregistered design

- Preferred, null and competing hypotheses:
- Predictions and falsifier:
- Primary contrast:
- Independent, dependent, nuisance and controlled variables:
- Reset tier and proof:
- Input determinism grade:
- Randomization, blocking, counterbalance or factorial plan:
- Positive manipulation, negative and boundary controls:
- Repetition/independence unit and stopping rule:
- Measurement algorithm, units, uncertainty and tolerance:
- Planned invalid/deviation case and expected disposition:

## Exact procedure

- Canonical commands in order:
- Expected runtime and hard timeout:
- CPU/RAM/GPU/port/resource bounds:
- Success predicate:
- Invalid-run predicate:
- Failure predicate:
- Cleanup and retention:
- Bounded retry:
- Stop/escalate when:

## Required return schema

- Packet ID/version/hash actually consumed:
- Worker identity and prior-context declaration:
- Start/end and environment/tool versions:
- Commands actually run and exit status:
- Run/capture/observation/measurement/protocol-deviation IDs:
- Immutable result hashes or terminal evidence:
- Valid, invalid, excluded and aborted runs with reasons:
- Observations, then separate interpretations:
- Values, units, tolerance and uncertainty:
- Hypothesis dispositions and contradictions:
- Smallest next experiment if unresolved:
- Files/processes retained or cleaned:
- Model/service/network/secret exposure declaration:
- `SUCCESS`, `INVALID`, `FAILURE`, `TIMEOUT`, `OOM`, `LOCKED` or `ESCALATE`:

The worker cannot approve its own finding, create an implementation transfer,
close the campaign/bead, or expand the question. A material packet change gets
a new version/hash and a fresh dispatch.

## Review and receipt

- Reviewer checks exact packet/result hashes, preregistration, deviations,
  independence, measurement validity, provenance and return-schema
  completeness.
- Review disposition: `APPROVE_EVIDENCE`, `REVISE`, `REJECT`, `PRIVATE_ONLY`
  or `ESCALATE`.
- Any transfer remains a separate artifact and review.

