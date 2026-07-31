# Level-One Goat family contract

Status: private Track P implementation contract  
Bead: `mc-dl0r.17.11.8.1`  
Scope: Oliver's exact qualified Level 1 owner-data package only

## Decision

The first live non-player family is the Goat family: class `5`, model `1`.
All 33 reset instances share one behavior implementation. Entity `50` is the
focused golden probe; the complete family is:

`50, 79, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97,
98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111`.

Goat is the smallest honest complete family for this milestone. The three
dolmens have a player regeneration effect but no source-backed visible self
change, class-11 records are invisible control entities, and the three
`villager-a` rows are only one skin subgroup of the 28-member villager
behavior family. Villagers additionally depend on building search and village
bookkeeping. The reset explosion/smoke/impact rows are transient capture-site
effects rather than a stable live family.

## Qualified reset identity

Every Goat row has:

- class/model/action `5/1/15`;
- parameter record `98`;
- particle parameter `238`;
- control flags `0x0000000c`;
- life/max life `600/600`;
- actual/minimum/maximum speed `18/54/18` in the recovered field names
  (`54` is the fast cap, `18` the slow floor);
- StageVar1/StageVar2 `3/2`;
- proximity countdown byte `32`;
- animation frame `0`; and
- a distinct deterministic RNG seed, position, phase, roll target, and source
  ID.

Stage variable `3` is the qualified type-2 point `(13568, 8192)`, with stage
flags `1`. StageVar2 `2` selects the bounded point/wander behavior. The
implementation must read these values from the validated package. They are
not replaceable tuning constants.

Parameter record `98` is retained as the exact 34-byte source record. Its
ordered decoded values are:

`39, 45, 5, 22, 5, 256, 0, -256, 20, 512, -1015554, 00 09, 32, 1536, 853, 9, 0`.

Field meaning remains limited to demonstrated consumers. In particular:

- subtype `45` is the bounded yaw-easing clamp; word 4 is passed through the
  same recovered helper but is unused by its current implementation;
- words 10, 12, and 14 feed terrain-height shaping;
- word 16 is the terrain/collision threshold;
- word 26 is a periodic divisor;
- word 28 is the attention/range radius;
- word 30 is an angle window; and
- byte 32 is a behavior flag set.

No friendlier names may imply stronger semantics than the recovered consumers.

## Source-ordered live path

One normal outer turn has this ordering:

1. The shared TMAP animation state advances before input and entity simulation.
2. Input and player update run, then `UpdateEntities_57730` advances the global
   RNG, reclaims disabled entities, rebuilds class/model lists, and runs stage
   maintenance.
3. The pre-dispatch `sub_68BF0` pass decrements each live Goat's nonzero
   proximity countdown byte and propagates it to linked children. At zero,
   `sub_68C70` first respects control-flag bit `0x01`, otherwise decrements the
   secondary countdown when nonzero, or re-arms the primary countdown to `16`
   when the local player is within strict squared distance `0x02400000`.
4. Active entities dispatch in ascending pool-slot order; action `15`
   dispatches to the Goat scripted update.
5. StageVar2 `2` compares each Goat with point `(13568, 8192)` using signed
   16-bit X/Y deltas and a `3072` threshold.
6. Outside the threshold, the common point-seeking path processes pending
   damage, terrain-aware motion, target yaw, same-family separation, collision
   response, and bounded yaw easing.
7. Inside the threshold, the common wander path processes pending damage,
   terrain-aware motion, and periodic RNG-driven yaw change.
8. The Goat-specific tail advances its 16-bit RNG as
   `seed = seed * 9377 + 9439` with wrapping arithmetic.
9. For action 15 (and actions 8, 9, and 11), `seed % 77 == 0` requests
   positional sound `46`. Action 14 instead vocalizes on `seed % 43 == 0`;
   action 10 redirects into the action-14 handler in the same pass.
10. A resulting action `14` selects the fast cap, raw `minSpeed` `54`; every
   other surviving action selects the slow floor, raw `maxSpeed` `18`.
11. `UpdateEntities_57730` increments the per-entity phase byte after dispatch.

The implementation must preserve this order because RNG, phase-gated steering,
sound emission, damage transitions, and the next state depend on it.

## Lifecycle band

The source action band is `8..15`:

| Action | Qualified role |
|---:|---|
| 8 | idle/common update plus Goat vocalization |
| 9 | roam/common update plus Goat vocalization |
| 10 | redirects immediately to hit/retaliation action 14 |
| 11 | linked/reaction update plus Goat vocalization |
| 12 | pre-kill transition |
| 13 | final kill/removal, normally producing mana |
| 14 | hit/retaliation pursuit and speed selection |
| 15 | authored StageVar/script update and speed selection |

StageVar2 `2` also performs source-driven player acquisition when the
pre-dispatch proximity countdown is nonzero and the phase is divisible by the
parameter record's cadence `32`. It scans eligible live players within squared
radius `1536^2` and angular difference `< 853`; the nearest match becomes the
target, StageVar2 becomes `10`, action becomes `14` because parameter byte 32
contains bit `0x08`, and the action-15 tail selects fast speed `54`. This
transition is part of the faithful boundary. Mode `10` engagement is marked
explicitly unsupported until its handler is ported; the transition must not be
silently suppressed merely because its destination is not yet live.

The milestone must prove live reset/action 15, private pending-damage injection
at the source field boundary, pre-kill, final removal, next-pass pool
reclamation, and reset restoration. The damage injection is a native
simulation qualification command with fixed attacker semantics; Unity may not
directly mutate Goat state. Full damage producers and arbitrary action-14
engagement remain outside this slice.

Action `13` removal is phase-gated. Mana conversion, the optional secondary
sphere, and drawing disable occur only when `(phase & 7) == 0`; action `13`
must remain live on other phases. Reclamation sets class to zero on the next
entity update.

## Presentation and audio selection

The audio sidecar distinguishes preparation from playback. Action `0` is a
pending `PrepareEventSound` row, not an audible command. Paused outer frames in
the qualified single-player/no-CD profile retain and coalesce those rows; the
first unpaused `PlayEntitySounds` call resolves them. Ordinary winners expose a
non-zero u64 flush ID (the minimum participating event sequence) and contiguous
one-based rank, while direct and
suppressed rows keep both fields zero. Sidecars remain queryable across bounded
event-journal expiry and arbitrary step backlog until the client explicitly
releases a resolved row; reset clears and invalidates the generation.

The qualified owner presentation group uses:

- TMAP bank `TMAPS1-0`;
- base selectors `152`, `153`, and `154`;
- orientation sector
  `(((entity_yaw - camera_yaw) >> 3) & 0xf0) >> 4`;
- lookup `0,0,0,1,1,1,2,2,2,2,2,1,1,1,0,0`;
- horizontal mirror for sectors `8..15`;
- four exact mutable animation states per selector, plus reset and wrap
  closure, qualified against the private remc2 oracle; and
- positional Goat sound `46`.

The orientation selector is camera-relative presentation derived from
Rust-owned Goat yaw. Selectors `152`, `153`, and `154` are independent shared
descriptor clocks, not per-entity clocks and not one clock for their common
allocation group. Rust owns each selector's source frame index,
payload-relative cursor, mutable indexed pixels, and preceding-draw used
latch. At the start of each unpaused outer turn it services selectors in
ascending order, clears each used latch before advancing, and applies the
exact `reset → F1 → F2 → F3 → F4 → F1` sequence. This includes the original
executable sequence beginning at `0x715F0`. Its `MOV [EAX],CL` write-back at
`0x715F3` is present in the pinned executable and mechanical source but is
accidentally omitted from current remc2. The private, hash-pinned LE mapping
and instruction proof is
`evidence/goat-tmap-latch-clear-0x715f0.json`, verified by
`tools/verify_goat_tmap_latch_clear.py`.
Repeated draws collapse to one advance; paused rendering preserves the latch.

The additive ABI exposes exact selector rows and accepts guarded authoritative
post-draw feedback. The original CPU draw-admission predicate is still a
separate qualification dependency, so normal Unity presentation submits no
feedback and deliberately remains on the exact base frame. Unity-local
animation remains forbidden.

Sound `46` is materialized in the private owner cache as the exact
`GOAT.WAV` record and uses the positional “play if not already playing” path. The
source varies its pitch by `+10..+29` while action 14 is active and `-10..+9`
otherwise. The pitch source is not the Goat RNG: it computes
`randVal = global_rng * 9377 + 9439` without storing `randVal` back, then uses
`randVal % 20 + 10` for action 14 or `randVal % 20 - 10` otherwise. At reset
slot 102 observes stored global RNG `0x9909f858`; the nonstored peek is
`0xa631b437`, giving exact action-14 `+25` and other-action `+5` pitch deltas.
The typed request binds and plays the exact owner asset without mutating the
authoritative RNG. Runtime sound ID `46` is one-based and therefore resolves
zero-based metadata slot `45`, `GOAT.WAV`, SHA-256
`6038ceffaa2c54f9d4f7a93494fc713bc6bc3b76bc9a8ecc7dfc7f3fe4445b18`;
metadata slot 46 is a different sample and must not be substituted. Extraction
must remain hash-bound to the owner closure and the sample must not be embedded
in Unity source or a distributable player.

## Authority and state ownership

- `NativeWorld` remains immutable and may be shared.
- Every native session owns its own 33-row mutable Goat state.
- Every cache-backed native session also owns three independent mutable
  selector-frame states and their used latches.
- Rust owns Goat position, yaw, roll target, speed, action, phase, proximity
  countdown and subcounter, RNG, life, active state, render selector, and
  lifecycle events.
- Unity reads snapshots/events and renders them; it owns no Goat gameplay
  clock.
- Villagers, dolmens, switches, and transient effects remain
  `FrozenObserved` and visibly unsupported.
- Reset reconstructs Goat state from the validated reset package.

The family/live-world hash is additive: existing player `state_hash`,
`source_projection_hash`, and `conformance_hash` values remain byte-for-byte
unchanged. Legacy `live_world_hash_v1` remains FNV-1a 64-bit over
`mc2-live-world-v1\0`, outer tick, Goat count and all Goat rows, house count
and all house rows, then villager count and all villager rows. Rows remain in
ascending entity-index order and include their active flags and every causally
mutable field, including roll, proximity countdown, and proximity subcounter,
as fixed-width little-endian bytes with explicit option tags. Global RNG
remains excluded from that byte-compatible v1 preimage.

Current direct and C ABI `live_world_hash` projections use the explicit
`mc2-live-world-v2\0` composite domain: the little-endian legacy v1 hash
followed by authoritative mission `D41A0_0.rand_0x8`. This admits RNG-only
mutations without duplicating RNG ownership or changing the ABI layout.
Selector animation remains excluded from both live-world domains; the separate
`selector_state_hash` covers descriptor state, mutable pixels, latches, and
accepted draw-feedback metadata. Generation, event cursors, audio voice
allocation, Unity transforms, and wall-clock state remain excluded.

The quiet Level 1 reset turn has this exact stored global-RNG sequence:

1. `UpdateEntities_57730`: `0xad6e67d9 -> 0x9909f858`;
2. model-60 slot 113: `0x9909f858 -> 0xa631b437`;
3. slot 114: `0xa631b437 -> 0x829a3776`;
4. slot 115: `0x829a3776 -> 0xd2c99e15`;
5. slot 116: `0xd2c99e15 -> 0xeb078414`;
6. slot 117: `0xeb078414 -> 0xde4d0573`;
7. slot 118: `0xde4d0573 -> 0xa334be32`;
8. slot 119: `0xa334be32 -> 0x0eeaca51`;
9. slot 120: `0x0eeaca51 -> 0x661cc5d0`;
10. slot 121: `0x661cc5d0 -> 0x43e9ccaf`.

The model-60 draw is source-driven, not an unconditional turn allowance:
terminal/inactive/non-model-60 rows omit it, while every live emitter stores
its draw before `NewEvent_4A050` chooses an ordinary or replaceable slot or
fails allocation. Builder, Archer, checkpoint, and ground-fire consumers
remain conditional at their own source call sites. Goat sound 46 observes the
stored `0x9909f858`; `PrepareEventSound_6E450` derives nonstored
`0xa631b437`, producing action-14 `+25` and other-action `+5` pitch deltas
without changing authoritative RNG.

## Evidence and qualification

Private source anchors:

- `EventsFunctions.cpp:1251-1258`: class-5 Goat action dispatch;
- `EventsFunctions.cpp:9977-10029`: StageVar dispatch;
- `EventsFunctions.cpp:10257-10342`: StageVar2 type-2 point/wander selection;
- `EventsFunctions.cpp:11386-11464`: Goat action behavior, RNG, sound, speed;
- `EventsFunctions.cpp:9532-9569`: pre-kill and kill wrappers;
- `EventsFunctions.cpp:26867-26916`: carried-mana conversion;
- `Events.cpp:5193-5239`: disabled-slot reclamation;
- `EventsFunctions.cpp:33728-33758`: constructor defaults;
- `EventsFunctions.cpp:39936-40183`: entity-pass ordering and phase increment;
- `EventsFunctions.cpp:55516-55590`: proximity countdown and local-player
  re-arm ordering;
- `EventsFunctions.cpp:44968-45033`: selector-global animation dispatch;
- `Sound.cpp:6254-6345,6426-6452`: positional Goat sound and pitch variation;
- `WavIndexes.h:50`: Goat sound identity;
- `runtime-action-annotations.csv:45-52`: reviewed action semantics;
- `creature-models.csv:3`: reviewed Goat model identity;
- owner cache `entities/presentation.json`, group `reset-050`: render closure.

Required verification:

1. exact reset projection for all 33 rows;
2. the preserved exact-build state sequence
   `sequence-002285FF-00356038.bin`: native reset/tick `0` must equal preserved
   frame `0`, and after native ticks `1..19` the state must equal preserved
   frames `1..19`, including index 50 pose/yaw/roll/countdown/phase/RNG and the
   complete 33-row family; this artifact does not contain a 20th transition,
   which requires an additional preserved frame `20`;
3. source-derived positive, boundary, rejection, and invariant vectors for
   StageVar2 `2`, RNG/sound, speed selection, and lifecycle;
4. deterministic same-command replay and stable per-tick family hashes;
5. reset restoration and two-session isolation;
6. exact-build discrimination for the focused probe where preserved evidence
   exists, with unsupported comparisons stated explicitly;
7. ABI layout and invalid-command rejection;
8. Unity EditMode/PlayMode proof that visible Goat changes originate in Rust;
9. owner-cache tamper/missing-file/privacy negatives; and
10. independent final review.

## Explicitly unresolved for this slice

The following remain absent or frozen unless separately proven:

- arbitrary attacker and combat targeting outside the qualification command;
- action-14/mode-10 engagement behavior beyond the proven acquisition
  transition boundary;
- complete mana-sphere economy and credit attribution after death;
- linked sub-entity damage propagation when a Goat has linked children;
- all StageVar2 modes other than the qualified Level 1 value `2`;
- the exact CPU draw-admission predicate that decides which qualified
  selector-local latch is set after presentation (the clock and its
  one-advance-per-unpaused-outer-turn service cadence are exact);
- global-RNG consumers outside the six reachable Level 1 stored-draw sites;
- whole-program interactions with non-Goat creatures, buildings, villages,
  objectives, or multiplayer; and
- public redistribution or successor transfer of reconstructed behavior or
  owner assets.

No unresolved branch may be filled with plausible steering, physics, VFX,
audio, animation, or gameplay and then labelled faithful.
