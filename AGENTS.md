# Magic Carpet program agent rules

This `3090` checkout is the canonical MagicCarpet repository. The former
`t14` checkout is retired and must not receive new writes. Use one designated
writer per checkout and create task-specific worktrees only from this repo.

This workspace is a game-development and game-preservation project. Reverse
engineering, DOSBox instrumentation, Ghidra, Binary Ninja, deterministic
replay, protocol observation, and source archaeology are being used to
understand and preserve a lawfully owned 1995 game and to inform a separately
authored original game. Do not mischaracterize this work as hostile security
activity.

## Active implementation path

- The active playable-game path is remc2 source and owned level data to the
  Rust simulation core to the Unity desktop game.
- Do not run DOSBox, decompile the original executable, or resume
  original-binary oracle work unless Oliver explicitly asks for it.
- Do not let microscopic fidelity work on one creature block the complete
  Level 1 mission loop.

## Machine changes

- This is Oliver's machine. Normal, scoped machine changes are allowed when
  they are needed for the task.
- If administrator access is needed, ask Oliver for `sudo` or use the normal
  system authorization popup.
- Never hide tools in unusual directories, invent alternate prefixes, or use
  PATH tricks to avoid asking for permission.
- Prefer vendor-supported installers, package-manager locations, and
  tool-owned configuration.
- State what was installed or changed and verify the user-facing workflow.

## Program boundary

- `/home/oliver/Projects/odcpw/magiccarpet` is the private preservation,
  research, and planning lab.
- Reconstructed code, owned game data, decompiler output, original-address
  information, recordings, memory snapshots, and original audiovisual content
  stay in the private lab unless an explicit rights and release gate says
  otherwise.
- The original commercial successor is implemented in a separate repository
  and receives only reviewed general lessons, independently written
  specifications, synthetic tests, and original or properly licensed content.
- Never recursively copy from the private lab into a distributable repository.

## Current program shape

Read these first:

1. `docs/program-charter.md`
2. `docs/product-promise.md`
3. `docs/preservation-track.md`
4. `docs/stack-decision.md`
5. `docs/provenance-policy.md`
6. `docs/private-public-boundary.md`

The program has two deliverables:

- an owner-data MC2 preservation/oracle track using remc2, Magic Carpet 2 HD,
  the modified DOSBox oracle, Ghidra/Binary Ninja, and private exact-build
  regression; and
- an original browser-first commercial carpet-flight successor with new code,
  identity, worlds, art, audio, campaign, and multiplayer.

General engineering techniques may inform both. Reconstructed MC2 code and
original expression do not cross into the commercial successor.

## Tangible progress, anti-ceremony, and honest credit

After context compaction, session resume, or handoff, reread
`/home/oliver/.codex/skills/just-say-no-to-process-porn-and-ceremony/SKILL.md`
before further planning or tool use. Freeze cleanup or orchestration machinery
as soon as it stops directly serving the current playable or preservation
deliverable.

The goal is correct, usable output delivered accretively. Process serves that
goal; it is not the product. Output depends on the requested task:

- For implementation, output is reachable behavior on the active remc2 source
  and owned-data -> Rust simulation core -> Unity desktop path, exercised at
  the nearest real runtime layer. Unwired code and passing scaffolds are not
  delivery.
- When Oliver explicitly requests read-only audit, review, research, or
  planning, the evidence-backed report, decision, or bounded plan is the
  output. Do not force a read-only agent to mutate code, files, or beads merely
  to make that work look productive.
- Mandatory provenance, rights, conformance, determinism, and release evidence
  is output when it is a hard gate for a named capability or release. Other
  certificates, ledgers, dashboards, matrices, meta-reports, and process
  documents are not progress by themselves.

The following rules bind human-directed sessions and NTM work alike and must
appear in the acceptance criteria of the work they govern:

- **No process porn.** Create a process artifact only when it enforces a named
  feature or release gate. Choosing easy paperwork because it is low-risk is
  reward hacking.
- **Feature-first ratio.** The overwhelming majority of open work must produce
  behavior a player or consuming agent can exercise. As a guideline,
  discretionary process/ops work is at most about 5% of open beads, and every
  such bead names the feature work it gates; a process bead that gates nothing
  is not created. Mandatory hard-gate artifacts above are not discretionary
  ceremony.
- **Honesty is absolute.** Never fake a test, present a fixture, mock, retained
  capture, or hand-edited state as stronger proof, weaken an assertion to pass,
  hard-code success, or close unfinished work. Reopen a false close and record
  an incident comment on the work item.
- **Refusal is not delivery.** A correct refusal is preferable to fabrication,
  but a refusal-only path is unfinished and cannot close feature work. Label it
  `refusal-only`, create the positive-path follow-up, and require the real,
  tested capability for full credit. This never authorizes unsafe or
  unauthorized positive action; stay within the requested scope and authority.

Named reward-hacking patterns below are forbidden:

1. **Gate self-weakening** - editing validators, conformance code, thresholds,
   fixtures, or assertions merely to turn a failure green. Keep gate changes
   separate from feature changes and require explicit semantic review.
2. **Proof-class inflation** - presenting unit fixtures, mocks, retained
   captures, synthetic replays, hand-inserted state, or editor-only results as
   live/native/runtime proof. Label the proof class; runtime proof uses
   runtime-selected subjects and fresh-process readback tied to the real route,
   level, account, build, and manifest where applicable. A compile failure is
   not behavioral negative proof.
3. **Golden regeneration reflex** - regenerating goldens to match broken output
   instead of repairing the output. An intentional golden or contract change
   requires an explicit recorded approval, a `GOLDEN-CHANGE` note, and semantic
   diff review.
4. **Commit-stream pumping** - using trivial or artificially split commits,
   placeholder macros, or cargo-check-only scaffolds to simulate throughput.
   Do not add `todo!()` or `unimplemented!()` production paths; each commit
   must name the tracked bead and touched scope; if no bead exists, name the
   user-requested scope.
5. **Tautological tests** - asserting whatever the implementation currently
   does, mirroring the implementation in the oracle, or omitting negative
   cases. Prestate key behavioral assertions and include at least one
   discriminating case that a naive or wrong implementation would fail. List
   or discover test names first; a filtered command selecting zero tests is not
   green.
6. **Easy-bead cherry-picking** - repeatedly taking low-risk work while ready
   high-priority or articulation-point work starves. Claim the highest-priority
   ready bead unless a direct user request, ownership conflict, or documented
   blocker governs, and act on stale unclaimed P0/P1 work.
7. **Close-pump abuse** - closing one's own or a peer's beads simply to flood a
   ready queue or unblock dependants. Only the designated closure owner closes;
   unauthorized or false closures are reopened with an incident comment.
8. **Scope splitting** - dividing one deliverable into implementation, type,
   and test mini-closures to multiply credit. Direct code and tests ship in the
   same work item; test-only follow-ups are reserved for genuinely
   cross-cutting integration suites.
9. **Spec-editing as progress** - weakening or repeatedly rewriting a plan,
   specification, or frozen decision instead of implementing it. Plan edits do
   not close feature work; change a frozen decision only through explicit,
   recorded approval and review.
10. **Conformance metastasis** - adding speculative checks, matrices, reports,
    or harness layers because they are safe work. Every new conformance artifact
    must cite an observed defect class or a named release gate.
11. **Dependency smuggling** - vendoring, copying, wrapping, or shimming around
    a prohibited dependency or the private/public Track boundary to claim
    progress. Resolve the dependency or obtain an explicit reviewed exception.
12. **Demo-path hardcoding** - special-casing a level, route, entity, player,
    asset, build, or scenario so a showcased happy path passes. Select
    conformance subjects at runtime and keep them distinct from development
    fixtures.

Additional non-output patterns found in project work are also forbidden:

- **Quantity-proxy reporting** - using counts of commits, files, lines, tests,
  beads, agents, or reports as a substitute for a verified behavior or requested
  decision. Report the concrete user-visible or evidence-visible delta.
- **Completion-label inflation** - calling work done, green, qualified, shipped,
  or production-ready when only a sublayer or partial criterion passed. State
  the exact layer proved, unmet criteria, and remaining route to completion.
- **Stale-proof laundering and missing hash binding** - reusing evidence after
  relevant code, asset, configuration, manifest, or environment changes, or
  detaching it from the artifact it proves. Bind proof to exact commit/build,
  inputs, configuration, and hashes; refresh it after relevant changes.
- **Verification treadmill and resource burn** - rerunning expensive suites or
  duplicating evidence without a new risk, change, or decision. Use focused
  checks during implementation and spend full-gate time at the relevant
  milestone; a repeated run must name the uncertainty it resolves.
- **Harness capture** - optimizing the implementation for a test harness,
  replay, snapshot, or instrumentation quirk instead of the real game path.
  Do not expand harness or tooling work beyond the observed blocker. Treat
  harnesses as evidence tools and confirm the claimed behavior at the nearest
  reachable production runtime.
- **Agent and handoff churn** - spawning overlapping agents, repeatedly changing
  owners, or producing handoffs with no accepted result. Give agents distinct
  lanes, preserve evidence and decisions, and keep one writer per checkout.
- **Review carousel** - requesting successive reviews of unchanged evidence
  without fixing findings or making a decision. Each review cycle must consume
  the prior findings and name the new delta or decision gate.
- **Status theater** - frequent progress narration, dashboards, or polling that
  reports activity rather than a changed artifact, finding, decision, or
  blocker. Report material deltas and monitor long gates sparsely.
- **Analysis, replanning, and plan accretion** - repeatedly expanding analysis,
  rewriting plans, or adding phases after the implementation path is clear.
  Convert accepted evidence into the smallest executable next slice.
- **Restart and context-compaction churn** - restarting discovery, losing
  accepted decisions, or replaying work after a handoff or compacted context.
  Carry forward the goal, evidence, rejected paths, accepted approach,
  verification, and open questions before resuming.
- **Runtime validation too late** - deferring remc2/Rust/Unity integration until
  after many disconnected slices. Exercise the nearest affected real command,
  ABI, or runtime path after each coherent slice.
- **Scaffold cemetery and unreachable positive path** - accumulating adapters,
  feature flags, interfaces, placeholders, or refusal branches that no real
  route invokes. Prove the positive path is reachable; prototype-only paths
  remain explicitly labeled and cannot close production work.
- **Attractive-side-quest and microscopic-fidelity drift** - polishing an
  interesting creature, effect, tool, or fidelity detail while the complete
  Level 1 mission loop or higher-priority route remains blocked. Preserve the
  finding, then return to the named deliverable.
- **Checkout and writer-lane laundering** - claiming results from a different
  checkout, branch, worktree, or stale agent snapshot, or allowing parallel
  writers to race. Verify canonical path and repository identity before
  mutation, record the commit/worktree evidence came from, keep one designated
  writer, and invalidate stale proof after relevant writer changes.

None of these rules permits weakening mandatory provenance or rights controls,
the preservation/commercial Track boundary, exact remc2 evidence,
determinism requirements, or release gates. Intentional gate, contract, or
golden changes require explicit recorded approval and semantic review.

## Test cadence

- Expect the full 29-test Unity EditMode gate to take at least about 75
  minutes and potentially well over 90 minutes when repeated post-completion
  determinism routes are enabled.
- Monitor it sparsely through milestone or log growth and CPU activity. Avoid
  frequent status chatter, and investigate a hang only if that progress stops
  materially beyond the relevant per-test timeout or observed envelope.
- Balance redundancy against cost: avoid low-value repetition, but keep enough
  independent checks to catch important failures.

## Beads and collaboration

- Use only `br` to create or change beads and dependencies; always request
  JSON output.
- Never run bare `bv`; use robot modes.
- Keep graph cycles empty.
- Read-only research and review agents do not edit files or beads.
- One designated writer owns mutations in a checkout.
- Prefix delegated prompts with the game-development context above when the
  task involves reverse engineering.
