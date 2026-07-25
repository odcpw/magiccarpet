# Behavioral reverse-engineering protocol

Version: 0.3  
Status: accepted private preservation and successor lesson-transfer protocol  
Tracking bead: `mc-w58.3`  
Depends on: program charter v1.0, provenance policy v0.5  
Decision date: 2026-07-23

## Purpose

Use the exact owned Magic Carpet 2 build, remc2/MC2HD reconstruction, modified
DOSBox oracle, Ghidra/Binary Ninja, controlled play, documents, recordings and
other registered sources to preserve and understand the game. MC1 is not a
research, parity, or implementation input.

Track P may produce exact private preservation findings, source correlations,
regressions, fixes and modernization experiments. Track S may receive only an
independently written, testable, provenance-reviewed functional lesson or
general method. Reconstructed source is valid Track P evidence; it is never a
Track S implementation dependency.

The preservation target is complete catalog/campaign understanding and
regression coverage. The commercial successor has no 25-realm or catalog
parity requirement.

Ghidra is a first-class research path. It is used deliberately, early and
deeply when static structure, data flow, update order, numerics or edge cases
cannot be established efficiently from observation. It is never treated as an
infallible transcript: decompiler output, guessed function boundaries and
recovered types are hypotheses until correlated with the running owned build.

Every successor-transfer campaign follows:

`Question → preregistered hypotheses and experiment → controlled observation
→ static/dynamic correlation → independent functional finding → synthetic
verification → provenance review → transfer → receipt`

The protocol must let a fresh analyst or fast delegated agent answer one
bounded question reproducibly, preserve uncertainty, and stop before Track P
evidence contaminates the original commercial successor.

## Track-specific outputs

### Track P preservation

May retain exact commits, addresses, symbols, code references, private
snapshots, recordings, original labels and owned data under the private
boundary. It classifies changes as fidelity correction, portability repair,
presentation modernization, tooling, deliberate deviation or unresolved
experiment.

### Track S lesson transfer

May receive only general techniques, externally observable high-level rules,
independently written measurements/specifications, and independently invented
synthetic vectors accepted under this protocol. It rejects source-derived
algorithms, control flow, structures/layouts, constants/tables, names,
recordings, snapshots, packet formats, original fixtures and audiovisual
expression.

## What this gate does and does not authorize

Closing this gate approves the method. It does not:

- install Ghidra, Glaurung, debuggers, extensions or Unity merely by closing
  this document's gate;
- start a behavioral executable-analysis campaign before the physical
  private-lab boundary is accepted by `mc-t8x` and the reproducible toolchain
  is accepted by `mc-51m`; after `mc-t8x`, `mc-51m` may perform only the
  minimal owned-build import, mapping and capture smoke tests explicitly
  required to validate that toolchain, producing no subsystem finding or
  transfer;
- create a public repository or describe the current root as a legal clean
  room;
- allow original or third-party binaries, captures, decompilation or assets to
  reach remote models/services;
- approve copying an external source port, extracted data, network format,
  code structure, names, tables or audiovisual expression into Track S;
- resolve contractual or statutory questions or approve release;
- permit an implementation worker to open raw analyzer evidence.

All original material, reconstructed-source analysis and raw analysis remain
in the private reference lab.
All machine changes use normal native/tool-owned install locations. If
privilege is genuinely required, ask Oliver or use the system authorization
prompt; do not improvise hidden prefixes, wrapper state or unusual directories.

## Governing sources and tool posture

- [`docs/provenance-policy.md`](provenance-policy.md) is controlling for source
  classes, traffic lights, physical separation, reviewers and transfer.
- [`docs/provenance/private-analysis-campaign-template.md`](provenance/private-analysis-campaign-template.md)
  is the campaign record.
- [`docs/provenance/analysis-transfer-template.md`](provenance/analysis-transfer-template.md)
  is the only executable-analysis handoff format.
- `research/private/source-register.md` is the private source/tool registry.
- Ghidra is the baseline static-analysis candidate. `mc-51m` pins and verifies
  the exact release, JDK, loader/extensions, analysis options and dynamic path.
  Current official documentation exposes GUI, headless/PyGhidra and debugger
  surfaces; their existence is not proof that the selected DOS executable
  loads or debugs correctly.
- Glaurung is an optional challenger controlled by `mc-fxwg`. At pinned commit
  `db227144364fe0859d44a3a9691b7ee83508ad0d`, it offers an automation-first
  Rust/Python/SQLite workflow but has no explicit DOS MZ/LE/LX loader in the
  reviewed tree, remains version `0.1.0`, and wires agent features to external
  model providers with automatic parent `.env` discovery. It receives no owned
  input unless a local no-egress synthetic-fixture audit passes.
- `PUB-DECOMP-WIKI-D049CCFD`, the
  [Decompilation Wiki](https://decompilation.wiki/) pinned at
  `d049ccfd672475778c7a800c006f29ebb7807d64`, is a useful taxonomy and
  primary-paper index for disassembly, function discovery, lifting, type
  recovery, structuring and quality evaluation. It is methodology evidence,
  not a source of Magic Carpet behavior. Claims taken from it must be traced
  to the cited primary paper when they materially alter this protocol.
- Public source ports and editors remain question generators unless their exact
  revision, license and lineage receive separate approval. Their identifiers,
  structures, tables and tests never become implementation shortcuts.

Decompiler presentation quality is not behavioral proof. The wiki's own
quality survey notes that overall decompilation quality has no single settled
metric; our acceptance therefore measures loader correctness, instruction and
function recovery, data/control flow, semantic predictions and runtime
behavior—not merely fewer gotos or prettier C.

## Knowledge model

### Separate these records

| Record | Meaning | May contain private analyzer detail? | Can drive implementation? |
|---|---|---:|---:|
| `BUILD_MANIFEST` | Exact owned inner payload and launch-chain identity | Yes; private only | As an opaque source ID |
| `ENVIRONMENT` / `CALIBRATION` | Frozen runtime plus passed input/clock/capture checks | Yes; private only | Constraints only |
| `QUESTION` | One falsifiable behavioral unknown | No need | No |
| `EXPERIMENT` / `RUN` | Preregistered contrast and one reset execution | Yes; private only | No |
| `WORKER_PACKET` | Immutable bounded dispatch for one question/experiment | May reference private IDs only | No |
| `MEASUREMENT` | Derived value, method, units and uncertainty | Yes; private only | Not alone |
| `HYPOTHESIS` | A possible behavior or mechanism with a falsifier | Private record may | No |
| `CAPTURE` | Immutable raw input/output evidence | Yes; private only | No |
| `OBSERVATION` | What occurred, without mechanism claim | Private reference only | Not alone |
| `MECHANISM` | Static/dynamic explanation of why it occurred | Yes; private only | No |
| `FUNCTIONAL_FINDING` | Freshly worded input/state/output rule | No Red detail | Only after acceptance |
| `CONTRADICTION` | Evidence incompatible with a live claim/hypothesis | Private references only | Blocks affected claim |
| `PROTOCOL_DEVIATION` | Procedure departure and confirmatory-validity disposition | Yes; private only | No |
| `SYNTHETIC_VECTOR` | Independently constructed tiny conformance case | No original payload | Yes after review |
| `TRANSFER` | Reviewed, immutable implementation package | Public-safe candidate | Yes, named destination only |
| `REVIEW` / `RECEIPT` | Independent disposition and exact downstream consumption | No Red detail | Governance only |
| `PRODUCT_DEVIATION` | Deliberate modern behavior relative to accepted reference | No private evidence | Yes after product approval |

Do not turn an interpretation into an observation. “The carpet moved 14 screen
pixels over ten captured frames” is an observation. “Velocity uses exponential
damping” is a hypothesis until discriminating inputs and analyzer/runtime
evidence support it.

### Record lifecycles

Record types do not masquerade as states of one object:

- question: `OPEN → ANSWERED` or `DEFERRED`;
- experiment: `DRAFT → PREREGISTERED → RUNNING → COMPLETE`, or `INVALID` /
  `EXPLORATORY`;
- hypothesis: `OPEN → SUPPORTED → CORROBORATED`, or `FALSIFIED` /
  `UNRESOLVED` / `SCOPE_SPLIT` / `SUPERSEDED` / `ABANDONED`;
- functional finding: `DRAFT → CORROBORATED → REVIEWED → ACCEPTED`, or
  `CONTRADICTED` / `REJECTED` / `SUPERSEDED` / `REVOKED`;
- transfer: `DRAFT → IN_REVIEW → APPROVED → CONSUMED`, or `REVISE` /
  `PRIVATE_ONLY` / `LEGAL_REVIEW` / `SUPERSEDED` / `REVOKED`;
- receipt: `CURRENT`, `STALE_RETEST`, `STALE_SUPERSEDED` or `STALE_REVOKED`.

`SUPPORTED` means the preregistered observation supports the hypothesis;
mechanism may remain unknown. `CORROBORATED` requires a genuinely different
evidence route. `ACCEPTED` means an independent reviewer approves the
functional wording, classification, evidence, uncertainty and verification.
`REJECTED` preserves the failed candidate and reason; `SUPERSEDED` points to a
reviewed replacement; `REVOKED` invalidates prior approval without deleting
history. An unresolved contradiction blocks the affected finding. No state is
inferred from evidence axes alone.

### Evidence axes are not status

Every hypothesis and functional finding carries five independent evidence axes.
Do not collapse them to a single confidence adjective:

| Axis | 0 | 1 | 2 | 3 |
|---|---|---|---|---|
| `O` observation | None | Uncontrolled or single valid run | Controlled repeated observation with boundaries | Exact-build, calibrated, preregistered observation with uncertainty and discriminating boundaries |
| `R` reproduction | None | Approximate or analyst-only reproduction | Independent exact-build reproduction of the scoped behavior | Reproduction across all named modes/builds/conditions with reset and negative controls |
| `T` triangulation | None | One evidence route | Two materially independent routes | Three routes including runtime correlation where mechanism or ordering is material |
| `A` applicability | Unknown | One explicitly bounded build/mode/condition scope | Named variations tested or a defensible scope boundary | All claimed variants exercised with version/mode differences recorded |
| `C` contradiction | No known contradiction | Resolved, immaterial or demonstrably non-overlapping scope | Material unresolved contradiction | Claim falsified inside its declared scope |

`C` is a disposition axis, not “more is better.” `C2` blocks acceptance and
transfer; `C3` falsifies the claim. `C1` is acceptable only when the resolution
or scope separation is explicit and reviewed.

Green transfer floors are:

- simple bounded rule: `O2/R2/T2/A1/C0–C1`;
- timing-, determinism-, network-, economy- or ordering-critical rule:
  `O3/R3/T2/A1/C0–C1`.

The axes express evidence strength, not workflow state. A finding can meet a
numeric floor and still be `CORROBORATED`, `CONTRADICTED` or `REVOKED`. A single
decompiler view, source-port assertion, manual simplification, longplay,
memory value or agent vote cannot establish acceptance.

### Fidelity classification

Every functional finding separately labels:

- `REFERENCE_FACT`;
- `REFERENCE_HYPOTHESIS`;
- `PROVISIONAL_REPLICA_RULE`;
- `DELIBERATE_MODERNIZATION`;
- `ORIGINAL_ADDITION`;
- `OMITTED`.

This prevents a practical demo decision—such as the provisional
`BanishAndExit` multiplayer profile—from being described as original behavior
before it is measured.

## Identifiers and immutable artifacts

Identifiers are stable and never reused:

- `SRC-<class>-NNNN` or an existing source-register ID: registered owned or
  public source;
- `BLD-<product>-NNNN`: exact inner-executable `BuildManifest`;
- `ENV-<host>-NNNN`: frozen emulator/runtime environment;
- `CAL-<ENV>-NNN`: clock, input or capture calibration;
- `TOOL-<name>-NNNN`: pinned executable/tool identity;
- `CAM-<BLD-or-SYNTH>-<subsystem>-NNNN`: bounded campaign;
- `Q-<CAM>-NN`: question;
- `EXP-<CAM>-NNN`: preregistered experiment;
- `PKT-<scope>-NNNN`: immutable bounded worker packet;
- `RUN-<EXP>-NNN`: one independently reset execution;
- `CAP-<RUN>-NN`: immutable capture bundle;
- `OBS-<RUN>-NN`: observation;
- `MEAS-<RUN>-NN`: derived measurement with method/uncertainty;
- `HYP-<CAM>-NNN`: hypothesis;
- `MEC-<CAM>-NNN`: private mechanism explanation;
- `FND-<CAM>-NNN`: functional finding;
- `CTR-<CAM>-NNN`: contradiction;
- `PDEV-<EXP>-NN`: protocol deviation and validity disposition;
- `DDEV-<scope>-NNN`: deliberate product deviation;
- `VEC-<CAM>-NNN`: synthetic vector;
- `TRN-<subsystem>-NNNN`: reviewed transfer;
- `REV-<TRN>-NN`: independent review and disposition;
- `RCPT-<TRN>-NN`: destination implementation receipt.

The implementation task keeps its canonical `br` bead ID; the evidence graph
references that ID instead of inventing a parallel task namespace.

`mc-51m` instantiates the canonical append-only, schema-validated evidence
graph:

`Source + BuildManifest + Environment + Calibration → Campaign → Question → Hypothesis → Experiment → Worker Packet? → Run + Protocol Deviation? → Capture + Observation + Measurement → Mechanism? + Contradiction* → Finding → Synthetic Vector + Review → Transfer → Implementation Task → Receipt`

Edges are typed `supports`, `contradicts`, `falsifies`, `refines`,
`supersedes`, `rejects`, `revokes`, `verifies`, `implements`, `invalidates` or
`requires_retest`.
Corrections append a new version linked to the old one; IDs and history are
never deleted or silently reused. The logical record for each artifact
includes:

`Finding` is the canonical registry entity and uses `FND-*`. “Claim” elsewhere
in this protocol is ordinary prose about an assertion, not a second entity.

- ID, schema version, author, reviewer, timestamps and status history;
- bead/campaign/question and parent evidence IDs;
- source IDs and exact owned-build/config/tool/project hashes;
- private classification and logical evidence reference;
- subsystem, game mode, level/test setup and applicability bounds;
- inputs, controls, outputs, units, sampling and uncertainty;
- observed fact, inferred mechanism, alternatives and falsifier;
- evidence-axis scores and evidence-route tags;
- contradictions, supersedes/revokes links and downstream consumers;
- data/tool/model exposure and retention/deletion disposition;
- immutable content hash.

The registry is append-only at the event layer. Corrections append a new
version plus `supersedes`, `contradicts`, `rejects` or `revokes` edges; they do
not rewrite prior evidence or status history. A material entity version records
its predecessor hash. Derived human-readable Markdown is a view, not the
authority. Registry validation rejects dangling lineage, reused IDs, mutable
content under an existing hash, self-approval and a destination receipt that
still consumes a revoked or superseded transfer.

Invalidation is fail-closed. A falsified, superseded, revoked or
`requires_retest` record is traversed through every dependent edge. Affected
findings and transfers become blocked, revoked or explicitly
`RETEST_REQUIRED`; receipts consuming their exact transfer/vector hashes
become stale; consuming beads are notified and cannot close while stale. Work
resumes only from a superseding independently reviewed transfer and a fresh
exact-hash receipt. The registry retains the entire fan-out and resolution
history.

Raw evidence uses a manifest and hashes. Renaming or moving a raw file does not
create new evidence; changing content does. Analyzer projects are versioned
snapshots or exports with a recorded tool/import/analyzer configuration—not
opaque mutable “latest” projects.

## Roles and write ownership

| Role | Access | May write | Must not |
|---|---|---|---|
| Campaign lead | Campaign scope and private registry | Campaign plan/status | Self-approve transfer |
| Observation analyst | Owned runtime/captures for assigned experiment | Own private experiment records | Browse unrelated analyzer evidence |
| Binary analyst | Exact owned working copy and private analyzer project | Own campaign project/evidence log | Implement corresponding Amber-derived subsystem |
| Measurement analyst | Named captures and extraction method | Derived measurements/private scripts/results | Change hypothesis after seeing results without recording it |
| Engineering provenance reviewer | Required private evidence and candidate transfer | Review/disposition only | Author package or decide legal questions |
| Implementation worker | Approved transfers and original design only | Public implementation/tests | Open private paths, raw captures, addresses or decompilation |

One agent owns writes to one campaign directory/analyzer project at a time.
Read-only reviewers may work concurrently. Parallel campaigns use distinct
projects/directories and distinct questions; they do not share a mutable Ghidra
project or overwrite a common registry file. The current shared-agent private
workspace is not true analyst/implementer isolation.

A reviewer must not be the author and must be able to make an independent
judgment. An agent forked with the analyst's raw conversation, analyzer
interpretation or preferred conclusion cannot be the sole independent
reviewer, even if it did not edit the artifact.

## Campaign lifecycle

### 0. Intake and authority

Before any original file is opened in an analyzer, record:

- exact subsystem and one bounded behavioral question;
- why observation, documentation or existing accepted claims are insufficient;
- applicable owned source/build/version hashes and license-risk state;
- approved operations, analyst/reviewer and implementer-isolation requirement;
- tools/services/models, local/remote status and possible data egress;
- expected raw evidence, candidate functional output and forbidden outputs;
- time, storage and data-minimisation budget;
- stop/escalation conditions.

A campaign with “understand the whole executable” is rejected. Split it at an
observable boundary such as input sampling, velocity integration, Fireball hit
resolution, terrain kernel, carrier reservation or network victory.

### 1. Preregister the question

Write before observing the targeted result:

- current known facts and source/evidence grade;
- primary hypothesis and at least one plausible alternative;
- exact observable prediction that differs among alternatives;
- primary contrast, independent variable, dependent measure, nuisance and
  controlled variables;
- setup/reset procedure, input schedule, manipulation/positive control,
  negative/boundary controls and exclusion conditions;
- blocked, randomized or counterbalanced run order where drift/order can bias
  the contrast; baseline bracketing before/after each block;
- a preregistered factorial treatment when an interaction or update ordering,
  rather than one main effect, is the question;
- repetition/sample plan, independence unit, analysis method and precision,
  equivalence, power or sequential stopping target;
- expected units, precision and tolerance;
- falsifying result and next decision for every possible result.

Exploratory work is allowed but is labeled `EXPLORATORY`; it may generate a
question, never close one. Confirmatory experiments use a new preregistered ID.

### 2. Freeze and calibrate the environment

No campaign starts until `mc-ytu2` provides a `BuildManifest` that identifies
the inner gameplay executable and the complete
`installer → launcher/script → DOSBox → inner executable` chain. Installer
hashes alone are not executable identity. Before `mc-t8x` and `mc-51m` approve
the physical lab and tested loader, do not assume MZ, DOS/4GW, an extender,
processor mode, image base, overlay behavior, packing or self-modification.

Record and hash:

- game executable/data/config/save source, install version and working-copy
  status;
- host OS/kernel/hardware/power state, display/audio path and relevant load;
- emulator build, configuration, CPU core/cycles, frameskip, scaler, render
  backend, mixer/sample rate, input mapping and network settings;
- resolution, game detail/performance settings, control sensitivity and
  enabled accessibility/cheat/debug options;
- capture tools, frame/audio rates, codecs, timestamps and measured overhead;
- analyzer/debugger build, loader, processor/language/compiler assumptions,
  image base, relocations/overlays and analysis options.

Original files are immutable masters. Experiments use hash-verified working
copies. A patched or instrumented binary receives a new ID, exact byte-diff
record and `MODIFIED_DIAGNOSTIC` label; its behavior never substitutes for an
unmodified-build observation.

Calibration proves that:

- game speed is stable for the selected emulator settings;
- input pulse duration and capture timestamps mean what the experiment assumes;
- capture/debug instrumentation does not materially change the measured path;
- reset/savestate/checkpoint reuse returns to the same relevant state;
- low/base/high emulator cycle settings and at least two render/capture
  conditions distinguish simulation-time dependence from host/render artifacts
  for timing-sensitive claims.

Each `CAL` record names its metric, warm-up, pass/fail or equivalence threshold,
pre/post baseline, delivered frame/audio rates, instrumentation delta and
host-to-guest-to-capture clock-mapping uncertainty. Every low/base/high cycle
or render condition must itself pass the non-starvation and clock calibration;
one valid baseline does not validate the perturbations.

If the game is starved, audio-stretched, frameskipping unexpectedly, thermally
throttled or otherwise outside calibration, the run is invalid—not an
interesting behavioral sample.

### 3. Controlled black-box observation

Each experiment tests one primary contrast. Factorial treatments are allowed
only when preregistered to distinguish an interaction or update order. Record:

- exact initial state and how it was established;
- ordered key transitions, mouse deltas/buttons and timing basis;
- camera/position/target geometry needed to interpret output;
- complete output/capture hashes and run notes;
- whether a cheat, savestate, speed control or setup-only modification was used;
- every `PROTOCOL_DEVIATION` from the preregistered procedure and its validity
  disposition.

Default minimum evidence:

- discrete deterministic rule: three matching valid repetitions plus at least
  one boundary/negative case;
- timing/continuous response: a justified repeatability floor, normally at
  least ten independently reset valid runs per condition unless a
  preregistered precision/equivalence/power or sequential rule supports
  another number; report distribution, spread and capture quantization rather
  than a naked average;
- stochastic rule: a declared independent-sample unit and RNG epoch/seed plus
  an effect-size/error target, confidence interval or hypothesis test chosen
  before collection; collect until the preregistered power, precision or
  sequential stopping rule is satisfied, not an arbitrary universal count;
- version/mode-sensitive rule: reproduce in every build/mode the claim names,
  or narrow the claim.

These are floors, not permission to stop when noise or alternatives remain.
Frames from one continuous run are repeated measurements, not independent
repetitions.

Reset tiers are `COLD_LAUNCH`, `LEVEL_RESTART`, `CHECKPOINT` and `SAVESTATE`.
The experiment declares which state may persist at each tier. A checkpoint or
savestate is usable for confirmatory repetition only after equivalence to the
declared cold baseline is demonstrated for the measured state and hidden RNG
epoch. Replaying clones of one savestate, measuring many frames from one run,
or measuring one event several ways is pseudoreplication, not independent
sampling. Stochastic inference uses distinct or explicitly logged RNG
epochs/seeds.

An unplanned procedure change, post-hoc exclusion, changed stopping rule or
changed analysis invalidates a confirmatory run or reclassifies it
`EXPLORATORY`, unless it falls inside a preregistered tolerance. Planned,
valid, invalid, excluded and aborted runs all remain indexed with reasons.

Input determinism is graded explicitly:

- `D0 MANUAL_UNTIMED`: human input without a trustworthy event clock; useful
  only for exploration or coarse discrete rules;
- `D1 MANUAL_TIMED`: captured human input with measurable scheduling error;
- `D2 REPLAYED_HOST`: repeatable host-side event sequence whose guest arrival
  jitter has been calibrated;
- `D3 GUEST_CORRELATED`: repeatable input correlated to the relevant guest
  update/timer boundary.

The campaign chooses the minimum acceptable grade before collection. Timing,
update-order and near-boundary claims normally require `D2` or `D3`. Raising
the grade after a failed run creates a new experiment version; it does not
retroactively validate earlier samples.

### 4. Static analysis

Every `BuildManifest` has an untouched baseline import. Each campaign clones
that baseline into its own project or versioned snapshot; two analysts never
write the same analyzer project.

Static work proceeds in bounded passes, retaining the analyzer log/config after
each:

1. **Format/loader pass:** distinguish outer MZ stub from payload; survey
   container, extender/host, real/protected-mode transitions, entry points,
   segments/selectors and selector bases, memory map, relocations, overlay
   identity/residency/generation, runtime-copied or unpacked code,
   self-modification, imports/interrupts and unexplained bytes. Validate the
   imported memory map and representative bytes/instructions against the
   unmodified runtime, recording segment:offset and selector-to-linear
   mappings. Any unresolved format, loader, relocation, packing, runtime-copy,
   self-modification or overlay ambiguity blocks every static semantic
   conclusion for the affected region and becomes a separate child
   investigation.
2. **Instruction/function pass:** recursive traversal and targeted linear
   review, code/data boundary checks, indirect calls/jumps, jump tables,
   tail-calls, shared epilogues, function chunks and false-positive functions.
3. **Runtime boundary pass:** startup, main/event loop, time sources, input
   sampling, renderer/audio calls, allocator, save/network boundaries and
   shutdown.
4. **Data/numeric pass:** integer widths/sign, fixed/floating point, units,
   coordinate frames, tables versus structures, overflow, rounding, clamps,
   RNG state and iteration ordering.
5. **Subsystem slice:** start from black-box observable events and trace only
   the readers/writers/callers needed to discriminate the registered
   hypotheses.
6. **Cross-reference pass:** enumerate all callers/readers/writers and verify
   that an apparently central routine is not a presentation-only or
   mode-specific path.

Every mechanism claim retains a minimal dataflow chain:

`input/source → normalization/dispatch → state reads → update → state writes →
observable sink`

Stop and escalate when runtime and static memory maps cannot reconcile,
multiple loaders remain plausible, an overlay/packing/self-modification path is
ambiguous, or unexplained bytes can materially alter the bounded claim.

Analyst names are neutral hypotheses such as
`hyp_velocity_integrator_03`, not assumed original semantics. Every name/type
records who/what set it, confidence, evidence and revision. Imported names or
signatures from a public reconstruction are prohibited unless a later explicit
audit approves a narrow question-generating use; they never make an owned-build
finding true.

Treat decompiler output as a lossy view. Validate critical decisions against
instructions, p-code/IR, CFG, def-use/xrefs, calling convention and runtime
effects. A cast, sign extension, alias, switch recovery, loop boundary or
structure field that changes the behavioral claim receives explicit review.

### 5. Dynamic correlation

Use the least invasive local mechanism selected by `mc-51m`: emulator debugger,
guest-aware trace, breakpoint/watchpoint, memory sampling or synchronized
runtime/analyzer navigation.

For each probe:

- identify the black-box event it should predict;
- record unmodified/modified build, segment:offset and selector-to-linear
  mapping, overlay identity/residency/generation and trigger;
- capture before/after values with widths, signedness and units as hypotheses;
- vary input to distinguish causation from correlation;
- repeat without the probe when timing or ordering could be perturbed;
- relate static writers/readers to the externally visible result.

A watched value moving with speed does not prove it is authoritative velocity.
A function called on impact does not prove it applies damage. Confirm through
discriminating inputs, downstream state/output and alternative-path tests.

### 6. Synthesize the independent functional rule

Write the smallest externally useful rule in fresh project language:

- inputs and observable/authoritative state;
- preconditions and validation;
- state transition/equation/truth table;
- exact update order and time basis where material;
- units, bounds, rounding/overflow and failure behavior;
- cross-system effects;
- build/mode applicability;
- deliberate unknowns and acceptable provisional choice;
- what implementation structure is explicitly *not* required.

Do not preserve analyzer function order, data layout, identifiers, comments or
decompiler phrasing. Bulk extracted constants and distinctive tables are not
made safe by reformatting.

### 7. Verify and transfer

An accepted rule needs:

- a genuinely independent corroboration route: documentation plus observation,
  static plus dynamic, two different controlled setups, or another route
  approved in the campaign;
- boundary/negative tests and no unresolved contradiction inside its scope;
- synthetic positive, boundary/rejection and invariant vectors covering every
  material branch of the transferred rule;
- for every `BLACK_BOX_MEASURED` or `EXTRACTED` rule, successful discriminating
  black-box reproduction against the exact unmodified owned build;
- for `DOCUMENTED` or `ORIGINAL` rules only, an explicit reviewed exemption
  from exact-build reproduction with its rationale and narrower applicability;
- traffic-light classification for every claim/constant/vector;
- independent engineering provenance review;
- immutable transfer package hash and named destination task/files.

The reviewer can return `APPROVE`, `REVISE`, `PRIVATE_ONLY` or `LEGAL_REVIEW`.
Placement in `research/transfer/` is not approval. Any content change invalidates
the reviewed hash.

### 8. Close, retain and trigger retest

Closeout records:

- questions answered/rejected/unresolved;
- contradiction disposition and evidence-axis state;
- private artifacts retained with purpose/retention;
- temporary downloads/captures deleted with confirmation;
- transfers produced, destination receipts and reviewer decisions;
- analyzer/project/tool/source hashes;
- exact rerun triggers.

Rerun on executable/data/config hash change, emulator/tool/loader/analyzer
change, contradictory observation, fidelity-profile change, affected transfer
revision, or implementation conformance failure that exposes missing behavior.

## Timing, numerics and uncertainty

Timing-sensitive campaigns track five clocks separately:

1. host monotonic/wall time;
2. emulator CPU cycles/core policy;
3. guest game update/timer event;
4. presented/captured video frame;
5. audio sample clock.

Never infer an authoritative tick from a screen recording alone. Use controlled
cycle/frame perturbations and, where possible, a correlated private runtime
trace. Report:

- sample count and warm-up;
- clock and event definitions;
- capture/frame/sample resolution;
- median, minimum/maximum and robust spread such as MAD or declared
  percentiles;
- missed/duplicated frames and interpolation;
- input scheduling error;
- debugger/capture overhead;
- whether a value is exact, quantized, fitted or bounded.

Multiplayer campaigns record an `ENV` and passed `CAL` for every instance,
topology/transport and host/client role, a common start marker or measured
clock mapping, capture synchronization uncertainty, and observed
latency/jitter/loss/reordering. Causal event order is stated per vantage point;
cross-instance order is not asserted more precisely than the clock/network
uncertainty permits.

`cycles=auto` or `cycles=max` may be an exploratory condition but cannot be the
sole basis of a precise timing claim. Report frame-quantized event times as
interval-censored ranges unless a finer calibrated clock resolves them.

For numerical mechanisms:

- test zero, sign changes, near-boundary values, saturation and overflow;
- distinguish world coordinates, camera coordinates, terrain indices and
  presentation pixels;
- identify integer/fixed/floating representation only after evidence;
- vary frame/cycle conditions to distinguish per-update from per-time behavior;
- retain competing models until an input makes different predictions;
- avoid transferring a full extracted table when a measured range, functional
  relation or small independently chosen vector is enough.

## Subsystem experiment matrix

Leaf beads own exact scripts; every one must inherit these discriminating axes.

| Subsystem | Minimum black-box axes | Analyzer/dynamic questions | Public-safe output |
|---|---|---|---|
| Flight | Step/pulse/reversal input; accel/coast/brake; forward/strafe/vertical; speed/altitude; surface proximity; collision; low/base/high cycles/frame conditions | Input sample boundary, velocity state, integration order, damping/clamps, terrain/water query, recovery | Coordinate/unit contract, equations/ranges, ordered synthetic traces |
| Camera/aim | Resolution/FOV; bank/turn/vertical input; stationary/moving shots; target offset/range; projectile visibility/hit | Camera versus carpet transform, crosshair/assist path, projectile origin/direction, render-only lag | Transform/targeting semantics, error bounds, synthetic geometry |
| Terrain | Known isolated edits; centre/edge/corner; repeated/simultaneous casts; water/limits; structures/actors | Storage/indexing, coordinate conversion, kernel, clamps, order, grounding and query invalidation | Abstract heightfield operations, invariants, tiny invented grids |
| Mana/Possession | Neutral/owned/exposed/carried/secured/spent; loss/reclaim; exact before/after totals | Ledger/state transitions, owner field, costs, carrier reservation/delivery, stronghold release | Conservation state machine, equations and synthetic ledger vectors |
| Spells/combat | Cast accept/reject; self/ally/enemy/neutral target classes; direct/AoE; tier; terrain/water; simultaneous hit | Dispatch, target filter, cost/cooldown, collision, effect order, damage/status/experience | Target/effect truth tables, ordered state transitions, bounded constants |
| Stronghold/carrier/death | Build/grow/damage/destroy; carrier loss; death with/without anchor; rebuild/elimination | Ownership, capacity, spawn/recovery state, orphan cleanup, resource release | Recovery/ownership state machines and conservation tests |
| Actors/AI | Detection boundaries; occlusion; target alternatives; attack/retreat; difficulty; resource scarcity | Perception cadence, scheduler/order, RNG, navigation, cooldown, shared/cheat state | Decision interfaces, capability profiles, distributions—not copied scripts |
| Objectives/progression | One predicate at a time; ordering; failure; exit; save/reload; secret; campaign versus network | Queue/dispatch, marker/exit activation, save categories, result ordering | Objective-family schemas and original synthetic chains |
| HUD/map/audio/music | State/event toggles; distance/direction/occlusion; priority/load; menu/map/focus; day/weather/biome | Cue dispatch/state exposure, information filtering, channel/priority policy | Event-to-semantic-cue contract; no original media, text, layout or melody |
| Multiplayer | Two+ isolated instances; per-instance ENV/CAL and common marker; topology/role; latency/jitter/loss/reorder; player slots; self/opponent effects; death/rebuild; disconnect; varied terminal predicates; map/message open | Relation/damage filter, network state categories, per-vantage event order, pause behavior, win/exit/objective loading | Functional relationship/session rules, causal uncertainty and synthetic network scenarios |

Cheats are allowed only to create setup state and are recorded. A rule observed
under a cheat is reconfirmed without it, or its scope explicitly says the cheat
may have affected the result.

## Contradictions and negative evidence

Contradictions are deliverables, not annoyances.

Create a `CONTRADICTION` record when sources or runs disagree. Do not average,
vote or silently select the nostalgic answer. Test, in order:

1. executable/data/version/region differences;
2. single-player/network/difficulty/level/script differences;
3. emulator cycles/core/timing/input/capture differences;
4. stale save/config/upgrade/state;
5. presentation versus authoritative state;
6. random/ordering effects;
7. manual simplification, public-source lineage or analyzer error.

An unresolved contradiction narrows or blocks the claim. A falsified hypothesis
stays indexed with its evidence so a later agent does not repeat it. “Nothing
happened” is useful only when the setup proves the event could have been
observed and the negative boundary is stated.

Contradiction dispositions are `OPEN_BLOCKING`, `NOT_OVERLAPPING_SCOPE`,
`VERSION_OR_MODE_SPLIT`, `MEASUREMENT_ERROR`, `HYPOTHESIS_FALSIFIED` or
`UNRESOLVED_BLOCKING`. The registry preserves the evidence and dependent
invalidation/retest edges for every disposition.

## Transfer and contamination controls

### Green

Fresh observable rules, measured ranges with uncertainty, independently
derived equations/state machines, abstract schemas, synthetic vectors and
original `PRODUCT_DEVIATION` records may transfer after review.

### Amber

Exact constants, distinctive ordering/table selections, extracted-only
behavior, file/network formats and short original terminology require
item-by-item necessity/minimality, independent rewrite/re-measurement and legal
disposition. Prefer a Green behavioral test or range.

### Red

No binary/data/manual/save/level, screenshot/video/audio, memory/packet dump,
address/symbol map, disassembly/pseudocode, analyzer comment, copied table,
source-port structure/test or original expressive asset/text transfers.

A synthetic vector must be invented from the functional finding. Retyping a
captured state, packet, level, save, table or memory layout is not synthesis.

The implementation worker receives:

- approved transfer ID/version/hash;
- functional findings and applicability;
- synthetic vectors/properties;
- uncertainties and permitted provisional choices;
- browser/server/determinism constraints;
- required behavior and explicit internal freedom.

It does not receive the campaign, analyst conversation, private path, address,
raw capture or analyzer project.

## Fast-agent worker packet

Every delegated packet is small enough to complete and review independently.
Target one question and normally 45–90 minutes of focused work, with a hard
180-minute stop. A static slice normally covers no more than about ten
candidate functions and three competing type/mechanism hypotheses.
[`docs/provenance/experiment-worker-packet-template.md`](provenance/experiment-worker-packet-template.md)
is the required schema-backed research specialization of the project-wide
`mc-pyin` packet; its exact commands later enter the `mc-3sy6` command catalog.

### Input packet

- campaign/question/experiment IDs and one sentence outcome;
- packet schema/version/hash, exact cwd and worker/reviewer context declaration;
- exact source/build/config/tool IDs, never an ambiguous “latest”;
- immutable input hashes, allowed files/operations, denied paths/services and
  one file/project ownership lock;
- hypothesis/alternatives/falsifier;
- setup/reset/input schedule and controlled variables;
- measurements, units, sample/repetition plan and tolerance;
- output templates and private/public classification;
- time/data budget and stop/escalation rules;
- exact commands, expected runtime/timeout/resource bounds, success/invalid/
  failure predicates, cleanup and bounded retry;
- dependencies and the one downstream decision this evidence informs.

### Required return

- procedure actually run and `PROTOCOL_DEVIATION` dispositions;
- immutable capture/result hashes and private logical references;
- observations separated from interpretation;
- measured values with uncertainty;
- hypothesis disposition and alternatives;
- contradiction/unknown records;
- smallest next experiment if unresolved;
- deletion/retention and tool/model/egress declaration;
- terminal disposition from the packet enum;
- no transfer package unless explicitly assigned.

Fast agents may collect or classify evidence; acceptance and transfer need an
independent reviewer. Repeated low-quality agent agreement is not independent
corroboration when every agent saw the same evidence or inherited the same
conclusion.

`research/protocol-dry-runs/DRY-0001-packet.md` version 3 and
`research/protocol-dry-runs/DRY-0001-v3-result.md` are the original synthetic
known-source execution test for this gate. The review record must show exact
packet/fixture/environment/tool hashes, a fresh no-context worker, three
prescribed results, a preserved deliberate invalid run,
observations/measurements, zero prohibited exposure and independent
disposition. It proves packet operability only; it is not Magic Carpet evidence
or implementation input.

## Tool-quality bake-off

Before an analyzer/loader/decompiler informs an owned-build claim:

- pin exact source/release, artifact/dependency hashes, license/notices and
  install/remove procedure;
- run with no owned input on known-source synthetic fixtures compiled across
  relevant x86 modes/optimizations and file formats;
- include embedded data, awkward control flow, jump tables, indirect calls,
  fixed/floating math, aliasing and intentionally deceptive function
  boundaries;
- verify loader memory map/relocations/entry before scoring decompilation;
- compare disassembly coverage and false code, function boundaries, CFG/xrefs,
  data references, type recovery and semantic predictions to ground truth;
- measure determinism, analyst-correction provenance, exportability, runtime
  and memory;
- retain minimized failures and label unsupported formats rather than forcing a
  misleading import;
- prove filesystem/process/network/secret behavior and deny remote egress for
  private work.

The best tool for a slice may be Ghidra GUI, headless Ghidra, a debugger,
DOSBox observation or an audited challenger. No score chooses a tool globally.
Two analyzers agreeing is useful but not independent proof if they share the
same loader/disassembly error.

## Acceptance dry-runs

The protocol gate passes only after the method can describe, without performing
the private analysis now, how these representative campaigns would close:

1. **Flight damping:** pulse and release input; discriminate linear,
   exponential and clamped alternatives across cycle/frame conditions; correlate
   the state writer; output a unit-declared equation/range and invented trace.
2. **Fireball resolution:** vary target relationship/domain and boundary hits;
   correlate validation/collision/effect order; output a truth table and
   synthetic state transitions without original VFX/audio/constants in bulk.
3. **Terrain edit:** isolate a tiny edit, repeat near clamps/water/anchors and
   correlate storage/query invalidation; output an abstract kernel contract and
   invented small grid.
4. **Network terminal rule:** use controlled instances to independently vary
   death, castle, mana, objective, exit and time, then inspect only the paths
   needed to discriminate them; preserve “unknown” until the smallest trigger
   reproduces.
5. **Audio cue:** toggle one authoritative event across distance/occlusion/load,
   correlate cue dispatch, and output a semantic priority/spatial rule with no
   original sample or melody.
6. **Registry integrity and revocation:** validation rejects duplicate/reused
   IDs, missing source/upstream references, broken content/prior hashes,
   author self-approval, a sole reviewer contaminated by inherited analyst
   context, any Red vector, and consumption of a revoked or stale transfer.
   Falsifying a retained hypothesis must leave it searchable and automatically
   block or stale its dependent finding, transfer and receipt.

## Downstream hand-off

| Bead | Must consume from this protocol |
|---|---|
| `mc-t8x` | Upstream interface: physical paths/access/backup/retention and public boundary required before toolchain smoke or analysis |
| `mc-51m` | Exact toolchain, loader, dynamic path, registry/schema, synthetic bake-off and reproducible project |
| `mc-ytu2` | Source/build/config/control inventory and immutable hashes |
| `mc-w58.4` | Versioned baseline reference corpus, evidence IDs and capture/calibration lineage |
| `mc-go9b`, `mc-bxa8`, `mc-tdot`, `mc-2zx.1` | Preregistered controlled-observation packets and evidence bundles |
| `mc-w3x`, `mc-2zx` and child campaigns | Bounded static/dynamic pass order, evidence axes, contradiction and stop rules |
| `mc-6ya`, `mc-nyts` | Independent campaign/transfer/corpus audit criteria |
| `mc-ufv` | Finding state, fidelity labels and product-deviation ledger |
| `mc-fxwg` | Glaurung no-egress and known-source DOS-format bake-off |
| `mc-h2i` | Weather, terrain and atmosphere questions with bounded observation/analysis contracts |
| `mc-sd4`, `mc-147` | Source grading and explicit question/falsification records |
| Implementation/spec beads | Approved transfer package only; no raw analyst context |

## Required gate evidence

Accept this protocol when independent reviewers confirm:

1. black-box experiments control build, emulator, time, inputs, resets,
   repetitions, uncertainty and capture overhead;
2. Ghidra/static analysis is first-class but no decompiler claim bypasses
   loader, instruction/dataflow and runtime correlation;
3. observation, mechanism, functional finding, contradiction,
   `PROTOCOL_DEVIATION` and `PRODUCT_DEVIATION` are distinct;
4. evidence strength and acceptance require falsification, independent evidence and
   review rather than agent consensus;
5. every subsystem has a bounded experiment/analyzer path;
6. a fresh fast agent can execute one packet and return reproducible evidence;
7. exact constants/tables and synthetic vectors obey traffic-light/minimality
   rules;
8. tool data egress, secrets, external code and optional Glaurung use are
   gated;
9. transfers are immutable, revocable and sufficient for an isolated
   implementation worker;
10. no wording claims legal clean-room status, release approval, tool
    installation or completed executable analysis;
11. the append-only registry validates identity, lineage, review independence,
    contradiction disposition, revocation and destination receipts.

Closing this gate approves a research method. `mc-t8x` has independently
accepted the physical private/public boundary; backup remains `UNVERIFIED`, so
no owned file may be moved or deleted. `mc-51m` must still prove the exact
tool/runtime implementation before any campaign begins.
