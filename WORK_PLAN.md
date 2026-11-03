# Shared-engine delivery plan

Oliver resumed implementation on 2026-11-01 with two Terra 6.1/xhigh workers
and one Sol 6.1/xhigh integrator. Finish Castle/mana and Alliance end to end,
then Army and status spells. Commit, push and install working batches. The full
shared single-player engine remains the goal; trace remc2 into its real Rust
consumers and keep unfinished acceptance open.

Castle spell/mana checkpoint delivered on 2026-11-02: runtime `f10d20c6`,
installed native `3bfe49a21bb4`. All three spell tiers, source failure/cleanup
controls, earned mana through deposit and paid spending, and the death-marker
merge/census path pass focused owner-data checks. The matching player loaded
the campaign map and NightSpire with the original HUD. Human Castle play,
hardware audio and performance still need Oliver's playtest.

`mc-bhq.10` remains open for its broader settlement/economy acceptance;
`mc-k5k.9` retains that blocking dependency. Their open status does not request
reimplementation of the delivered Castle paths.

Alliance checkpoint delivered on 2026-11-02: runtime `4acdee29`, installed
native `a620baf8396d`. The actual acquired three-tier cast/takeover path,
representative allied attack through real target damage, and natural expiry
pass the existing owner-data scenario. Focused controls cover special creature
consumers, current parent records, allocation failures/reuse, shared A4 input
and Castle relations, arrow collision, restoration, and replay/carry. The current
occupant determines scheduler dispatch, including a reused player address.
The matching installed player loaded the campaign map, original loading screen,
and NightSpire terrain/HUD; its native library and all 437 Rust inputs match the
tested candidate. No Unity or original-art changes were required.

`mc-k5k.5` stays open for its combined families22/23/24 acceptance. Its bounded
checks do not establish every eligible creature's complete mission playability.

Army checkpoint delivered on 2026-11-03: runtime `c83fe2f4`, installed native
`85fc99aca629`. The acquired spell's nine-case owner-data route passes: source
payment/cooldown/recast, 9/24 flight, 10/72 rings, all three tiers and the four
Bee/Firefly/Cymmerian/Wyvern products, actual attributed outgoing damage, and
representative natural expiry/death/split/landing/reclaim. The original eight
cases remain; real Cymmerian and Wyvern rival-damage checks were added.
Fixed current parent/A4/target reads, Firefly current-position/distance reads,
allocation-failure charging and the queued-profile zero-counter update order.
The installed player loaded the original campaign map, loading artwork and
NightSpire terrain/HUD; native copies, the loaded library and all 437 Rust inputs
match the tested candidate. No Unity or original-art changes were required.

The exact Army native and install receipt are preserved as the status batch's
rollback; the earlier Alliance rollback remains available. `mc-k5k.4` stays open for combined
families19/20/21 acceptance. Progression/resources and some positions are explicit
test controls, not earned campaign progression or a human playthrough. Hardware
audio/performance remain unverified; the package-wide format check still fails
on broad formatting drift.

Status checkpoint delivered on 2026-11-03: runtime `11b49428`, installed native
`d87e112bc161`. Acquired/public casts pass all supported Speed, Shield, Invisible
and Beyond Sight tiers through movement, genuine Bee damage, actual Nyphur
selection and observation, with natural expiry and the real Bee death/drop/reuse
route. Source fixed-record/A4 publication now reaches current typed and retained
actors; Shield checks current Castle funding, queued free profiles survive cleanup,
and Speed preserves post-trail-allocation alias/payment/expiry order. Typed i32
Speed multipliers reach actual public movement, reset, carry and same-revision
replay without changing MC2 defaults. The source Invisible holder/A4 anomaly and
Bee exception remain explicit; latched attacks are not universally erased.

Final immutable candidate9 reuses the byte-identical core8 binary. All 437 source
inputs match its receipt and the installed native. Focused controls, acquired
consumers, affected C ABI acquisition/damage/labels, workspace all-target check
and native build pass; independent checks cover the new lifetime boundaries and
exact C ABI Shield damage. C ABI vault rewards use declared controlled BUILD21/54
damage; Shield's accepted mailbox is not claimed as natural combat. The installed
player loaded the original map, loading art and NightSpire terrain/HUD with the
matching mapped library. No Unity or original-art changes were required.

Use this status installation as the current baseline. New definition keys reject
older-definition saves/replays; no migration or relaxed identity gate was added.
The exact Army native `85fc99aca629` and its receipt remain under
`research/private/generated/status-20261103-integration/candidate-6/rollback-army/`.
`mc-k5k.12` and parent `mc-k5k.7` retain their full acceptance and open dependencies.
The shared-effects dependency `mc-bhq.9` remains open, including an adjacent bare
reclaim-helper test failure reproduced on the pre-batch Army binary; its fixture
versus production-lifecycle cause remains unproven. Formatting is still non-clean;
UBS changed-hunk review found no critical candidate defect, not a clean whole-file
scan. Human flight/combat, hardware audio/performance and full missions remain
unverified. Evidence is in `research/private/generated/status-20261103-integration/`
and `research/private/remc2-unity-native/Builds/delivery-status-20261103-batch1/`.
This batch stops here.

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

1. Use the delivered Castle/mana, Alliance, Army and status checkpoints above.
   Other preserved work remains in scope for later batches. Check live source
   and ownership first; do not restart a component already delivered.
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
