# Private executable-analysis campaign

Track: `PRESERVATION`. This record does not authorize reconstructed code,
original data, binary-derived structures or raw evidence to enter Track S.

This record stays in the private lab. It may contain private source IDs and
evidence-log references, but raw code, screenshots, addresses, dumps, and
secrets belong in the campaign’s explicitly private evidence directory.
The append-only evidence registry defined by
`docs/behavioral-reverse-engineering-protocol.md` is authoritative; this
Markdown record is its human-readable campaign view.

## Campaign identity

- Campaign ID:
- Registry schema/entity version:
- Content SHA-256 / prior-version SHA-256:
- Bead:
- Version/status:
- Subsystem:
- Question ID/version/status:
- Exact behavioral question:
- Analyst:
- Engineering provenance reviewer:
- Started/completed:
- Prior exposure/conflicts:

## Sources and environment

- BuildManifest ID:
- Environment ID:
- Calibration IDs and pass/fail results:
- Owned source IDs and hashes:
- Applicable build/version:
- Inner executable/data/config/save hashes:
- Installer-to-launch chain, working directory, command line and mounts:
- OS/emulator/runtime:
- Effective core/cycles/frame/input/audio/game/network settings:
- Ghidra/debugger/tool versions:
- Import settings and project hash:
- Loader/processor/image-base/segment/overlay/relocation hypotheses:
- Storage directory/classification:
- Backup/retention:

## Tool, agent, and data exposure

- Agents/tools/models/services:
- Local or remote:
- Conversation/context inherited:
- Files/content exposed:
- Possible network/data egress:
- Approval basis:
- Deletion/retention:
- Implementer-isolation requirement:

## Scope and stop conditions

- Exact inputs:
- Approved private operations:
- Explicitly forbidden operations/output:
- Expected private evidence:
- Expected transfer package:
- Time/data limits:
- Stop/escalate when:

## Hypotheses and pre-registered experiments

### Hypothesis registry

For each hypothesis:

- Hypothesis ID/version/status:
- Parent Question ID/version:
- Falsifiable statement:
- Derivation and source/evidence IDs:
- Predicted observations:
- Supporting/contradicting/falsifying IDs:
- Discriminating experiment/falsifier:
- Disposition evidence:
- Supersedes/retest links:

Status is `OPEN`, `SUPPORTED`, `CORROBORATED`, `FALSIFIED`, `UNRESOLVED`,
`SCOPE_SPLIT`, `SUPERSEDED` or `ABANDONED`. Never delete a rejected or
falsified hypothesis.

### Experiment registry

For each experiment:

- Experiment ID/version/status:
- Null, preferred and competing hypothesis IDs:
- Predicted observations and falsification rule:
- Independent, dependent, controlled and nuisance variables:
- Initial state/reset proof:
- Input determinism grade and input-script hash:
- Positive/negative/boundary/adversarial cases:
- Randomisation/counterbalance/factorial plan:
- Repetitions or statistical stopping rule:
- Capture streams, clock/calibration IDs and tolerances:
- Measurement algorithm, units, uncertainty and equivalence margin:
- Exclusion/abort rules:
- Expected private bundle / permitted transfer output:

## Evidence log

For every experiment:

- Experiment/run/observation/measurement IDs and time:
- Protocol-deviation IDs and validity disposition:
- BuildManifest/environment/input/capture hashes:
- Private evidence reference:
- Controlled input and environment:
- Scheduled/actual input and directly observed output:
- Measurements, units, uncertainty and dropped/invalid run accounting:
- Analyst interpretation:
- Alternative explanations:
- Dynamic reproduction:
- Static hypothesis and dynamically-hit/unobserved status:
- Private mechanism IDs/status, without copying analyzer detail here:
- Experiment disposition:
- Evidence axes `O/R/T/A/C` and summary (use the definitions and `0`–`3`
  rubric in `docs/behavioral-reverse-engineering-protocol.md`):
- Falsifying test:
- Follow-up:

Keep direct observation separate from mechanism inference.

## Findings and contradictions

For every candidate finding:

- Functional finding ID/version/status:
- Private mechanism IDs supporting the finding:
- Fresh functional statement and scope:
- Supporting/contradicting/falsifying IDs:
- Observed behavior versus inferred mechanism:
- Build/mode/version applicability:
- Traffic light and Amber minimality/necessity where applicable:
- Required behavior / implementation freedom:
- Synthetic vector IDs:
- Transfer eligibility and destination:
- Re-test/revocation triggers:

For every contradiction:

- Contradiction ID/type/severity:
- Exact overlapping claims, environments and evidence:
- Candidate explanations:
- Discriminating experiment:
- Downstream block/narrow/supersede/revoke action:
- Resolution/status: `OPEN_BLOCKING` / `NOT_OVERLAPPING_SCOPE` /
  `VERSION_OR_MODE_SPLIT` / `MEASUREMENT_ERROR` /
  `HYPOTHESIS_FALSIFIED` / `UNRESOLVED_BLOCKING`

## Closeout

- Questions answered:
- Questions unresolved:
- Rejected/superseded hypotheses retained:
- Private artifacts retained:
- Temporary artifacts deleted:
- Transfer IDs produced:
- Reviewer decision:
- Dependent receipt/task status:
- Re-run triggers:
