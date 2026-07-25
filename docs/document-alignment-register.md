# Document alignment register

Date: 2026-07-23  
Authority: [`program-charter.md`](program-charter.md)

This register records the repository-wide reset from one “faithful public MC2
replacement” plan to two isolated tracks:

- **Track P — preservation:** private remc2/MC2HD validation, exact behavior
  research, owner-supplied original data, native and WebAssembly feasibility.
- **Track S — successor:** original commercial carpet-flight game, browser
  social demo first, authoritative multiplayer, Unity baseline, Blender
  pipeline, later native OpenXR and streamed destructible cities.

## Governing documents

| Document | Disposition |
|---|---|
| `program-charter.md` | Program authority and track boundary |
| `preservation-track.md` | Current Track P contract |
| `product-promise.md` | Current Track S product promise |
| `stack-decision.md` | Unity baseline and measured fallback gate |
| `successor-stack-pin.md` | Exact S0 tools, packages, architecture and hard proof budgets |
| `platform-portability.md` | Current platform contract |
| `browser-first-ux-contract.md` | First-session, accessibility, input and motion-comfort contract |
| `browser-lifecycle-and-reconnect-contract.md` | Focus, suspension, lease, repair and safe-carpet state machines |
| `prototype-distribution-and-licensing.md` | Private prototype, dependency, content and publication guardrails |
| `modular-engine-game-module-contract.md` | Current Track S runtime seams |
| `multiplayer-session-contract.md` | Current Track S session authority |
| `provenance-policy.md` | Shared boundary and transfer policy |
| `private-public-boundary.md` | Physical isolation and repository policy |

## Aligned supporting documents

| Area | Current interpretation |
|---|---|
| Behavioral reverse engineering | Track P exact preservation; only reviewed lessons transfer |
| Manual and source inventories | Track P coverage evidence, not the successor backlog |
| World representations and voxels | Track S provider architecture |
| Blender and asset tooling | Track S original-content pipeline |
| Native VR | Later Track S OpenXR adapter; browser VR is not required |
| Destructible cities | Track S second vertical slice after the island demo |
| Transfer packages | Track P-to-Track S lessons only, never reconstructed source or original assets |

## Archived decisions

The pre-reset product, platform, modular-engine and multiplayer documents are
preserved under `archive/2026-07-23-pre-remc2-reset/`. Their banners identify
them as historical. They are evidence of prior reasoning, not current
requirements.

## Repository topology

The current `magiccarpet` checkout remains the private lab and Beads control
plane. The pinned `upstream/magic-carpet-2-hd` clone is Track P only. The
successor will be created separately at `carpet-flight` after Git identity,
provenance and stack gates pass. A future distributable preservation project,
if ever approved, must also receive its own repository and release review.

## Consistency rule

Every active bead and new document must name exactly one owning track:
`track:preservation`, `track:successor`, or `track:shared`. A Track S bead may
depend on a reviewed lesson-transfer gate, but never on completion of the MC2
campaign, remc2 source, original assets or address-derived structures.

## Bead alignment snapshot

Validated 2026-07-23:

- 686 active issues: 498 successor, 183 preservation and 5 shared; the JSONL
  also retains 43 tombstones;
- zero missing or multiple track labels;
- 461 inherited direct Track P/Track S blocking edges removed;
- zero remaining direct preservation-to-successor or
  successor-to-preservation blockers;
- the 25 former replacement-realm implementation tasks are now Track P
  preservation regression tasks;
- `mc-yu03` owns the Unity/browser/server/Blender stack proof;
- `mc-w58.15` is the successor approval gate and does not depend on Track P;
- `mc-dl0r.12` is the preservation architecture gate and has no authority over
  the successor;
- `bv --robot-triage` data hash `10059d4357d4a3ee` found 686 nodes, 2,349
  edges, phase-two readiness and no cycles; and
- `mc-vgm`, `mc-cm9`, and `mc-5b8n` are now closed from the exact stack
  contract; `mc-9od` is the next actionable planning bead, while `mc-w58.15`
  remains the approval gate before installation or implementation.

`br dep cycles --blocking-only --json` exceeded its 55-second bound on this
graph after the current rewiring. This is
tracked as a tool-performance limitation, not treated as positive cycle
evidence; `bv` found no cycle. `br doctor --repair --json` rebuilt the stale
blocked cache, vacuumed the database and passed its post-repair verification.
