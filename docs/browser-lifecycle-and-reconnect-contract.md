# Browser lifecycle, reconnect, and safe-carpet contract

> **Deferred as of 2026-08-21.** This is optional browser/multiplayer client
> research downstream of the one-engine two-package proof; it cannot define
> simulation authority or current work selection.

Version: 1.0  
Status: deferred optional-client research
Owner bead: `mc-x8i`  
Related contracts:
[`browser-first-ux-contract.md`](browser-first-ux-contract.md),
[`multiplayer-session-contract.md`](multiplayer-session-contract.md), and
[`successor-stack-pin.md`](successor-stack-pin.md)

## Outcome

Losing focus, pointer lock, visibility, connectivity or the entire browser
process cannot leave stale controls running, duplicate an authoritative
identity, pause a competitive world, create invulnerability or silently lose
the player's route back into the match.

The server remains authoritative even when the browser emits no final event.
Browser lifecycle signals accelerate safe handling; they are never the sole
safety mechanism.

## Non-negotiable invariants

1. At most one `ControllerLease` can issue commands for one active member.
2. Input captured before focus/capture loss cannot be replayed afterward.
3. Every gameplay command carries member, lease epoch, sequence and intended
   tick.
4. Duplicate, stale, wrong-epoch and already-applied commands have explicit
   deterministic outcomes.
5. Focus loss never pauses the authoritative match in the S1 rules profiles.
6. Safe-carpet mode remains visible, targetable, damageable and strategically
   inferior to active play.
7. Safe-carpet mode cannot cast, claim, collect, spend, bank, complete an
   interaction or receive a hidden-information advantage.
8. Reconnect resumes the same member or fails explicitly; it never creates a
   replacement member implicitly.
9. Client cache, timers, `online` state, unload handlers and WebSocket state are
   advisory. Server tick and lease state decide authority.
10. Repair completion requires an accepted checkpoint plus ordered tail and
    matching canonical hash.
11. Returning from hidden/frozen state never resumes prediction or command
    emission before repair/admission completes.
12. No lifecycle or reconnect log contains room tokens, reconnect secrets, IP
    addresses or raw input history.

## Separate state machines

Browser presentation, transport and authoritative member control are separate.
One state change never implies that the other layers already changed.

### Browser presentation state

```text
Boot
-> Landing
-> Preflight
-> Loading
-> Lobby
-> Ready
-> Captured
<-> OverlayVisible
-> HiddenOrFrozen
-> ReturnPending
-> Repairing
-> Captured
-> Result
```

`FatalCompatibility`, `RoomUnavailable`, and `ReconnectExpired` are explicit
side exits with recovery actions.

### Transport state

```text
Disconnected
-> Connecting
-> Authenticating
-> Synchronizing
-> Live
-> Suspect
-> Reconnecting
-> Repairing
-> Live
```

The browser may report a live socket while the application heartbeat is
`Suspect`. The application state wins.

### Authoritative controller state

```text
PendingAdmission
-> Active
-> InputSuspended
-> SafeCarpet
-> RepairHold
-> Active
-> Ended
```

`DisconnectedExpired`, `AITakeover`, and `Eliminated` are preset-defined exits.
Only server events move authoritative state.

## Lifecycle event normalization

The TypeScript shell converts browser events into an ordered
`ClientLifecycleEvent` stream with a local monotonic sequence:

| Browser signal | Normalized event | Required immediate action |
|---|---|---|
| window `blur` | `InputFocusLost` | Clear held actions and mouse deltas |
| pointer-lock loss | `CaptureLost` | Clear input; open recovery overlay |
| fullscreen exit | `FullscreenExited` | Preserve play only if capture/input remain valid |
| `visibilitychange` hidden | `DocumentHidden` | Clear input; request suspension; persist view/settings |
| `freeze` | `DocumentFreezing` | Close/release browser resources and stop presentation work |
| `resume` | `DocumentResumed` | Enter return-pending; do not emit gameplay commands |
| `pageshow` persisted | `HistoryRestored` | Revalidate build, socket, lease and hashes |
| `pagehide` | `PageLeaving` | Best-effort suspension only |
| `document.wasDiscarded` | `PageWasDiscarded` | Full reload plus reconnect admission |
| browser offline hint | `NetworkHintOffline` | Mark suspect; heartbeat remains decisive |
| browser online hint | `NetworkHintOnline` | Attempt bounded reconnect; do not assume connectivity |
| controller removal | `InputDeviceRemoved` | Clear that device's actions |
| controller return | `InputDeviceAdded` | Require neutral state before enabling |
| WebSocket close/error | `TransportLost` | Stop new commands; begin reconnect state |
| heartbeat timeout | `AuthorityUnreachable` | Stop speculative side effects and reconnect |

`beforeunload` is used only for a genuine unsaved-user-work warning if such
work exists. `unload` is never a correctness mechanism.

## Immediate client input safety

On focus, pointer-lock, hidden, device-removal or transport-loss events:

1. Increment the local input generation.
2. Zero every held, analog, repeat, accumulated-mouse and queued-cast value.
3. Cancel unacknowledged presentation-only repeats.
4. Freeze prediction at the last permitted bound.
5. Emit one idempotent `SuspendControl` carrying the current lease epoch,
   last command sequence and lifecycle reason when transport permits.
6. Enter the non-authoritative recovery overlay.

The input sampler reads the current generation at the beginning and end of
each sample. If it changed, the entire sample is discarded. A command envelope
cannot be created from mixed pre-loss/post-loss input.

On return, all devices must pass through a neutral sample before they can
produce gameplay commands. Reacquiring pointer lock alone is not sufficient;
server admission/repair must also be complete.

## Server command freshness and suspension

- An accepted explicit `SuspendControl` enters `InputSuspended` on the next
  authoritative tick.
- The server independently enters `SafeCarpet` after no fresh eligible command
  for at most 15 authority ticks (`500 ms` at 30 Hz).
- A command arriving after suspension cannot reactivate control. It is rejected
  with `LeaseSuspended`.
- Only `ResumeControl` after successful lease and repair validation returns the
  member to `Active`.
- Continuous actions terminate when their last accepted command expires; they
  do not retain a browser key-down state.

The 500 ms fail-safe is measured under the declared impaired-network test.
If it produces false suspensions, command redundancy/acknowledgement is
improved; the game does not extend stale-control execution silently.

## Safe-carpet law

Safe-carpet is a deterministic neutral command profile, not AI pretending to
be the player.

It:

- ends casting, charging, targeting locks and interactions;
- disables collection, claiming, banking, spending and objective activation;
- zeros strafe, turn and vertical intent;
- reduces throttle toward a declared safe-coast value;
- applies the same ordinary damping, terrain collision and damage laws;
- may use only the minimum already-public terrain clearance response used by
  every uncontrolled carpet;
- remains visible, targetable, damageable and eligible for ordinary
  elimination;
- emits an opponent-visible disconnected/safe-state marker when the rules
  profile permits that information;
- receives no new perception or hidden state.

It does not:

- become invulnerable, intangible, invisible or non-colliding;
- teleport to a stronghold or leave the combat volume;
- evade projectiles or choose routes;
- gain altitude except through the universal imminent-grounding response;
- preserve a charge, combo, capture or channel;
- score, loot or progress an objective.

Safe-carpet behavior is hashed and replayed as ordinary authoritative state.

## Rules-profile outcomes

The same state machine supports each first-demo mode:

| Preset | During reconnect grace | At expiry |
|---|---|---|
| Solo versus AI | Safe-carpet; world continues | Declared AI takeover or ordinary loss |
| Human co-op | Safe-carpet; allies continue | Declared AI takeover or disconnected member loss |
| PvP | Safe-carpet; fully vulnerable | Ordinary elimination/forfeit rule |
| Mixed human/AI faction | Safe-carpet | Existing eligible AI controller may receive a new lease |

The S1 default reconnect grace is `45 seconds`. A rules profile may shorten it
but cannot hide the value. Expiry occurs on a server tick and is part of replay
and results.

AI takeover is a new controller lease with a new epoch. It never reuses the
browser lease or its queued commands.

## Reconnect identity and lease

### Stored client hints

The browser may persist:

- opaque session/match/member reference;
- last accepted checkpoint ID;
- last applied event/operation sequence;
- last canonical permitted-view hash;
- build/protocol/rules/content compatibility tuple;
- short-lived opaque reconnect credential.

The reconnect credential:

- is scoped to one member and match;
- is short-lived and revocable;
- is rotated after successful resume;
- is never placed in a URL, analytics event, console log or failure bundle;
- is insufficient to issue gameplay commands without a new lease.

### Reconnect admission

The client sends:

```text
ReconnectHello
  compatibility tuple
  session/match/member reference
  reconnect proof
  last checkpoint ID
  last applied sequences
  last permitted-view hash
  client lifecycle reason
  fresh client nonce
```

The server replies with exactly one:

- `ResumeFromTail`;
- `ResumeFromCheckpoint`;
- `FullPermittedSnapshot`;
- `MatchAlreadyEnded`;
- `ReconnectExpired`;
- `MemberReplacedOrRemoved`;
- `CompatibilityRejected`;
- `CredentialRejected`;
- `RoomUnavailable`.

No rejection silently creates a new room/member.

### Duplicate-session rule

At most one live lease exists per member. A valid reconnect:

1. increments the lease epoch;
2. atomically invalidates the old connection/lease;
3. records `LeaseReplaced`;
4. completes checkpoint/tail repair;
5. requires a neutral local input sample;
6. activates the new lease.

Late packets from the old lease are rejected by epoch before command
deduplication. Two tabs racing to reconnect produce one winner and one explicit
`LeaseReplaced` result.

## Checkpoint, tail and terrain repair

Repair uses:

1. compatibility validation;
2. a permitted-view checkpoint or full snapshot;
3. ordered command/event/world-edit tail;
4. authoritative current tick;
5. regional/subsystem hashes;
6. client acknowledgement of the resulting canonical hash.

The client:

- discards prediction and transient cosmetic effects beyond the repair base;
- applies snapshot and tail in canonical order;
- requests bounded regional repair for hash mismatch;
- does not accept gameplay input until the server confirms the repaired base;
- may render a noninteractive repair view without implying live control.

Large snapshots and terrain repair are chunked away from latency-sensitive
control traffic. Repair is idempotent: repeating a checkpoint or tail range
does not duplicate impacts, resource transfers, terrain edits, objectives or
results.

Hidden-tab return must repair within `5 seconds` on the reference impairment
profile. Full checkpoint/tail reconnect must restore control within
`10 seconds`, or show a progressing, cancellable recovery state and record a
hard-gate failure.

## Audio, fullscreen and pointer lock

- Audio lock is presentation-only; muted play remains valid.
- Audio resumes only after a user gesture and retains the player's audio
  settings.
- Fullscreen is optional. Its refusal never suspends authority if focus and
  input remain valid.
- Pointer lock is optional for controller/digital-turn modes but expected for
  standard mouse flight.
- Losing pointer lock always clears mouse/held input and requires deliberate
  resume.
- Repeated permission prompts are forbidden after refusal; the overlay offers
  alternative input and a single retry.

## OS sleep, process death and server restart

### OS sleep/process death

The client cannot be relied upon to signal these. Server command freshness
enters safe-carpet. On page reload:

- `document.wasDiscarded` and persisted hints explain the likely path;
- compatibility and lease admission run from the beginning;
- no cached socket or local `Active` state is trusted.

### Server restart

S0 may fail the match explicitly if server restore is not yet implemented.
It must still provide a stable result/error code and room return.

Once match restore is admitted:

- the restored authority loads a versioned checkpoint and committed tail;
- increments an authority epoch;
- rejects commands from the prior authority epoch;
- reconnecting clients receive the same repair protocol;
- incomplete restore ends the match explicitly rather than creating divergent
  authorities.

## Player-facing states

Every interruption overlay answers:

1. What happened?
2. Is the authoritative match still running?
3. What is the carpet doing?
4. How long is reconnect grace?
5. What is happening now?
6. What can the player do?

Examples:

- `Mouse control released. Your carpet is coasting and remains vulnerable.`
- `This tab was hidden. Checking the match and repairing state…`
- `Connection lost. Reconnecting — 31 seconds of grace remain.`
- `Another tab resumed this pilot. This tab no longer controls the carpet.`
- `The match ended while you were away. View results or return to the room.`

The UI never says `Paused` unless the server has entered an explicit
rules-profile pause state.

## Transition evidence

Every lifecycle transition records a redacted structured event:

```text
client lifecycle sequence and reason
client input generation
connection ID hash
lease epoch
last sent/accepted command sequence
server tick and member control state
checkpoint/tail IDs
hash before/after repair
duration and outcome
build/protocol/rules/content IDs
```

Browser wall time assists diagnostics but never orders authoritative events.

## Test matrix

### Deterministic model tests

- every state/event pair has an explicit accepted or rejected transition;
- repeated transition messages are idempotent;
- stale lease epochs and sequences are rejected;
- safe-carpet canonical hashes replay identically;
- grace expiry and AI takeover occur on exact ticks;
- snapshot/tail repetition cannot duplicate any conserved or world-edit event.

### Headed Chrome scenarios

1. Escape releases pointer lock during held movement and casting.
2. Window loses focus during held analog/controller input.
3. Tab hides for 1, 5, 15 and 60 seconds.
4. Tab freezes/resumes where browser tooling permits.
5. Tab is discarded/reloaded using `document.wasDiscarded`.
6. Back/forward-cache restore revalidates all authority state.
7. Browser process is killed without unload.
8. Network disconnects before and after a suspend message.
9. Wi-Fi/network changes while the socket appears open.
10. Controller disconnects with non-neutral axes.
11. Audio/fullscreen/pointer-lock permission is denied.
12. Two tabs race to resume one member.
13. Reconnect credential is stale, invalid or already rotated.
14. Build/protocol/rules/content identity changes during absence.
15. Checkpoint chunk, tail event or terrain repair is duplicated, delayed,
    reordered or corrupt.
16. Match ends or reconnect grace expires while hidden.
17. Server restarts with restore disabled and, later, enabled.
18. Four clients undergo staggered focus/network chaos under the declared
    impairment profile.

### Hard acceptance

- Local input clears in the same browser frame as the observed loss and before
  another command sample.
- Explicit suspension applies on the next server tick.
- Silent-client fail-safe enters safe-carpet within 15 authority ticks.
- No post-suspension gameplay command is accepted without a new active lease.
- Hidden return repairs within 5 seconds.
- Full reconnect restores control within 10 seconds.
- Exactly one lease controls a member after every duplicate-session race.
- No command, cast, resource transfer, terrain edit, objective or result is
  duplicated.
- Safe-carpet remains vulnerable and produces no progress.
- Client/server/replay hashes converge after repair.
- Failure artifacts include the first rejected transition or divergent hash
  without secrets or raw inputs.

## Sources

- [Chrome Page Lifecycle API](https://developer.chrome.com/docs/web-platform/page-lifecycle-api)
- [Chrome: deprecating the unload event](https://developer.chrome.com/docs/web-platform/deprecating-unload)
- [MDN Page Visibility API](https://developer.mozilla.org/en-US/docs/Web/API/Page_Visibility_API)
- [MDN Pointer Lock API](https://developer.mozilla.org/en-US/docs/Web/API/Pointer_Lock_API)
- [MDN WebSocket API](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket)
