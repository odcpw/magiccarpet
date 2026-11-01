# Shared-engine delivery plan

Oliver resumed implementation on 2026-11-01 with two Terra 6.1/xhigh workers
and one Sol 6.1/xhigh integrator. Finish Castle/mana and Alliance end to end,
then Army and status spells. Commit, push and install working batches. The full
shared single-player engine remains the goal; trace remc2 into its real Rust
consumers and keep unfinished acceptance open.

Read this plan and your current bead before each assignment, follow-up, resumed
work session or heartbeat. Also reread after compaction and before declaring a
handoff complete. Continue useful work; do not produce a read-receipt report.

## Outcome and scope

Deliver faithful MC2 single-player behavior in the reusable Rust engine.
Shared systems own behavior; missions configure and exercise them. Keep
properties and asset bindings typed, and behavior in focused canonical modules.
Use original presentation. Oliver does the playthroughs.

First finish the preserved 16 shared-engine gaps: spells 7 and 13–24,
Meteor 9 identity; creature models 10,16,17,18,21,23,25–28; authored spawning;
objectives 1,2,4,6 and rival targets; missing scenery/projectile/effect callbacks;
final encounter; secret entry/return and carry; player-info/mark/cycle controls;
campaign-map hover/cursor/terminal effects; original settings dialogs.
Then continue the remaining existing shared single-player engine/system beads.
Existing beads retain their full acceptance. Multiplayer, an authoring platform
and a level-content expansion are separate. Do not restart planning.

## Execute the existing plan

1. Finish the current connected batch: Castle/mana and Alliance, then Army and
   status spells through their actual gameplay consumers. Other preserved work
   remains in scope for later batches. Check live source and ownership first;
   do not restart a component already delivered.
2. Each feature owner follows the entire remc2-to-Rust chain: properties/assets,
   ordinary producer, schedule/dispatch, behavior and interactions, same-pass
   tail, following-pass cleanup, and actual gameplay/presentation consumers.
   Use the trace already in the bead and source. Implement missing links rather
   than writing another trace document.
3. A worker remains responsible through integration. A leaf handoff is a
   checkpoint, not completion or permission to abandon the feature. Integrators
   implement shared joins with the worker and return concrete failures to that
   owner. A missing dependency gets an implementation owner, not fake success.
4. Independent workers may finish other complete in-scope systems in parallel.
   Reserve disjoint files. One writer at a time owns each shared root; obtain an
   explicit edit window. Shared roots compose behavior rather than absorbing it.
5. At a coherent boundary, use the smallest checks that exercise the actual
   producer-to-consumer path. Reuse warm compilation; no workspace suite per
   tweak, bots, automated playthroughs, duplicate tests or weakened assertions.
   Passing a constructor test does not prove a creature or spell works.
6. Commit and push the coherent, verified implementation and install matching
   Rust/Unity artifacts. Run one delivery launch/load smoke. Deliver useful
   batches without waiting for all 16, while keeping unfinished beads open.
   Report separately what is source-ready, verified, committed and installed.
7. Continue the remaining systems, then use fresh bounded review of completed
   paths. Fix concrete findings. Claim completion only against original bead
   acceptance, with honest coverage limits and a matching installed build.

## Coordination and prompts

The current fleet uses Terra 6.1/xhigh workers and Sol 6.1/xhigh integrators.
Discover live NTM panes and Mail identities rather than trusting old names.
The current coordinator owns acceptance and tracker updates. The Sol integrator
implements shared joins and dispatches the sole Cargo/Unity build slot, coherent
commits/push and matching native/player installation. Discover actual identities
from the live session. Preserve dirty source, staged work, warm caches and current
installations.

- `engine-economy`: actual per-owner Castle lifecycle, directional/resource/late
  spells, shared damage/mana consumers. First remove singleton Castle production.
- `engine-creatures`: finish ordinary and summoned creature lifecycles and boss
  integration; prioritize missing model18/21/27 and the existing Army consumers.
- `engine-world`: real fire/projectile/effect descendants, fields/disasters,
  authored spawning, switches and objective evaluation. Implement prerequisites.
- `engine-delivery`: shared secret/session/save-carry routes, original controls
  and presentation/assets, matching native/Unity delivery. Keep one member on
  the existing opening consumer; no automated playthroughs.

Integrator ownership includes implementation of shared joins. Never hold a
shared-root reservation while waiting on another family: negotiate brief exact
file windows, integrate the coherent change and release. Workers remain with
their feature through the real consumer; do not accumulate unintegrated leaves.

For the current incremental build/install batches, check actual free bytes and
pending output size before expensive stages. The earlier blanket 20 GB hold was
replaced by the bounded headroom decision in mc-zdo.1: stop the next stage below
5 GB free or when its measured footprint exceeds available headroom. Preserve
the current player, immediate rollback and warm caches.

Every dispatch/follow-up must say: reread this plan and the bead; the positive
runtime result; the exact remaining link and owned surface; the integration
owner; the focused evidence and handoff boundary. Reference existing details
instead of repeating manuals or stale status. Use literal canonical `%N` pane
IDs and verify receipt through acknowledgment or actual work. Never print tokens.

The coordinator rereads this plan every tick, checks live work and messages,
and acts on specific blockers or idle actionable owners. No blind nudges or
status-only loops. Stay quiet on unchanged state. Preserve work and report once
if quota or access prevents progress. Stop the heartbeat only when the scoped
acceptance, fresh review and matching delivery are complete.
