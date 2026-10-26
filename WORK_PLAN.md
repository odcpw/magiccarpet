# Shared-engine delivery plan

Execution is paused at Oliver's 2026-10-26 wind-down request. The fleet is
stopped and the heartbeat is paused. Resume implementation only when Oliver
requests it; unfinished bead acceptance remains open.

Read this plan and your current bead before each assignment, follow-up, resumed
work session or heartbeat. Also reread after compaction and before declaring a
handoff complete. Continue useful work; do not produce a read-receipt report.

## Outcome and scope

Deliver faithful MC2 single-player behavior in the reusable Rust engine.
Shared systems own behavior; missions configure and exercise them. Keep
properties and asset bindings typed, and behavior in focused canonical modules.
Use original presentation. Oliver does the playthroughs.

Current scope is the agreed first 16 shared-engine gaps: spells 7 and 13–24,
Meteor 9 identity; creature models 10,16,17,18,21,23,25–28; authored spawning;
objectives 1,2,4,6 and rival targets; missing scenery/projectile/effect callbacks;
final encounter; secret entry/return and carry; player-info/mark/cycle controls;
campaign-map hover/cursor/terminal effects; original settings dialogs.
Existing beads retain their full acceptance. Multiplayer and an authoring
platform are separate. Do not restart planning or expand the scope.

## Execute the existing plan

1. Finish the connected batch already underway: Army and its actual summoned
   creature consumers, directional spells through impact and world effects,
   and secret entry/return with correct loading and preserved state. Check live
   source and ownership first; do not restart a component already delivered.
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

The current fleet uses Luna/max workers and Terra 6.1/high integrators. Discover
live NTM panes rather than trusting old pane lists. GreenFinch coordinates
acceptance, tracker and pushes. BrownCrane dispatches the sole Cargo/Unity build
slot. Preserve dirty source, staged work, warm caches and current installations.

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
