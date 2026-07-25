# Track P to Track S lesson-transfer package

This document is the only handoff from a Track P executable-analysis campaign
to a Track S implementation worker. It must stand alone without original code,
addresses, screenshots, copied tables, proprietary files, or decompiler
phrasing.

## Header

- Transfer ID:
- Registry schema/entity version:
- Package version:
- Package hash in transfer index:
- Prior-version hash:
- Supersedes:
- Revokes:
- Campaign bead:
- Source track: `PRESERVATION`
- Destination track: `SUCCESSOR`
- Subsystem:
- Question ID/version:
- Behavioral question:
- BuildManifest ID for the inner gameplay executable:
- Environment ID:
- Applicable game mode/level family/conditions:
- Analyst:
- Independent reviewer:
- Reviewer prior exposure/conflicts:
- Review reference and current disposition:
- Date:
- Relevant source IDs:
- Exact owned build hashes, referenced by BuildManifest/source IDs only:
- Publication classification: `TRANSFER_REVIEW`
- Tool/model/service exposure inherited from campaign:

## Functional finding

- Functional finding ID:
- Finding version/status:
- Traffic light: `GREEN` / `AMBER` / `RED`
- Derivation: `BLACK_BOX_MEASURED` / `DOCUMENTED` / `EXTRACTED` / `ORIGINAL`
- Observable inputs:
- Observable outputs:
- State:
- Preconditions:
- State transition:
- Ordering/timing:
- Bounds and failure behavior:
- Cross-system effects:
- Known version differences:
- Deliberate uncertainty:
- Supporting/contradicting/falsifying record IDs:
- Re-test/revocation triggers:

For every Amber finding:

- Minimality/necessity:
- Why a Green substitute is insufficient:
- Measured versus extracted detail:
- Independent rewrite/re-measurement:
- Legal-review disposition:

Describe the rule in fresh project language. Prefer equations, truth tables,
state diagrams, and abstract schemas over narrative that resembles analyzer
output.

## Evidence quality

- Directly observed behavior:
- Inferred mechanism:
- Alternative explanations:
- Dynamic reproduction:
- Evidence axes `O/R/T/A/C` and reader summary (use the definitions and
  `0`–`3` rubric in `docs/behavioral-reverse-engineering-protocol.md`):
- Falsifying experiment:
- Unresolved branches:

Private address/symbol/raw-trace references belong only in the analyst’s
private evidence log, not here.

## Synthetic verification

Every accepted rule needs a minimal positive vector, a boundary or rejection
vector, an invariant/property, and coverage of each material state-transition
branch. Determinism-critical or Amber vector authors should differ from the
private analyst.

- Vector ID:
- Traffic light: `GREEN` / `AMBER` / `RED`
- Construction method and author:
- Synthetic initial state:
- Synthetic command/input sequence:
- Expected state/output sequence:
- Tolerances:
- Edge cases:
- Property or invariant:
- Seed/version:
- Finding/transfer version and immutable hash:
- Material branches covered:

The vector must be independently constructed. Do not copy a save, level,
memory dump, original table, packet capture, or asset.

## Implementation freedom

- Required behavior:
- Why this lesson benefits the original successor:
- Evidence that the specification is independent of reconstructed internals:
- Explicitly non-required internal structure:
- Acceptable modernisation:
- Intentional deviation options:
- Performance/browser/server constraints:

## Transfer review

- [ ] No original/decompiled code, pseudocode, symbols, addresses, or comments.
- [ ] No copied data table, save, level, geometry, media, text, UI, or asset.
- [ ] No private filesystem path or unpublished credential.
- [ ] Wording and organization are independently authored.
- [ ] Numeric constants are measured/necessary and individually justified.
- [ ] Positive, boundary/rejection and invariant synthetic vectors cover every
      material branch of the accepted rule.
- [ ] Every executable-derived rule has a discriminating black-box reproduction
      against the exact unmodified owned build; a `DOCUMENTED` or `ORIGINAL`
      exemption is explicit, reviewed and justified.
- [ ] Sources, version, evidence axes, and disagreements are recorded.
- [ ] Reviewer is independent, discloses inherited context, and is not the
      author or a context-ineligible sole reviewer.
- [ ] Trademark, terminology, license, and release concerns are flagged.
- [ ] Reviewer approves transfer to the named implementation task.

## Decision

- Decision: `APPROVE` / `REVISE` / `PRIVATE_ONLY` / `LEGAL_REVIEW`
- Reviewer:
- Reviewer prior exposure/conflicts:
- Review reference:
- Date:
- Conditions:
- Approved destination task/files:
- Destination receipt/change ID:
- Approval history:
- Re-review/revocation trigger:

## Implementation receipt contract

The destination returns a separate receipt containing:

- receipt ID/schema/version/hash;
- exact transfer/vector IDs, versions and hashes consumed;
- destination task, files and change/build ID;
- implementer private-evidence/conversation exposure declaration;
- independent design/algorithm choices;
- accepted deviations and missing behavioral questions;
- focused vector/test commands and results;
- tool/model/service exposure;
- reviewer disposition;
- stale/retest state and triggers.
