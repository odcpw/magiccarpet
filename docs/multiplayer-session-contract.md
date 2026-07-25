# Original successor multiplayer session contract

Version: 1.0  
Status: successor-track rules and journey baseline  
Track: `S`

## Decision

The commercial successor uses a sole authoritative Linux room simulation.
Solo, co-op, PvP, teams and mixed human/AI games are rules/faction presets over
the same commands, world, resources, terrain, actors and result machinery.

Original MC2 LAN/NetBIOS behavior is private Track P evidence only. It is not
the successor transport or authority model.

## Identity

Stable identifiers include:

- `SessionId`, `MatchId`, `MemberId`, `ControllerId`;
- `FactionId`, `ActorId`, `WorldId`;
- `RulesProfileId`, `RulesHash`, `ContentRoot`;
- `SimulationBuildHash`, `ProtocolVersion`;
- reconnect token and checkpoint/tail identity.

Display names, browser connections, IP addresses and room creators are not
authority identities.

## Authority

The server owns:

- membership/factions/spawns;
- accepted commands and tick order;
- actor movement and abilities;
- health/effects/death/recovery;
- finite-resource custody;
- strongholds/reserves;
- creatures/rival AI;
- objectives/results;
- terrain/structure edits and hazards;
- RNG streams;
- permitted-view snapshots;
- reconnect/late-join checkpoints and repair.

Clients own input sampling, prediction, interpolation, camera, UI, audio,
cosmetic VFX/debris and accessibility presentation. They cannot author damage,
resource, objective, spawn, terrain or result state.

## Transport model

- Browser-compatible secure transport behind HTTPS.
- Versioned framed messages with hard size/count/rate limits.
- Commands carry intended tick/sequence and are acknowledged/rejected
  explicitly.
- Snapshots are relevance/permitted-view filtered.
- Ordered event/world-edit tails plus periodic regional/state hashes support
  repair.
- Application health messages expose RTT, loss/stall and server progress.
- No raw legacy NCB structures, original packet formats or direct peer TCP.

## Presets

### Solo expedition

One human faction, AI rival/ecology, no separate simulation implementation.

### Cooperative expedition

Humans share a faction. The rules profile explicitly defines shared resource,
stronghold/reserve access, damage filtering, objective credit, death/recovery,
disconnect and grief-prevention policy.

### Free-for-all

Each human/rival faction competes. Same-faction exceptions are irrelevant;
world edits and environmental hazards remain shared world state.

### Teams

Fixed faction membership for the match. No mutable diplomacy in the first
demo.

### Mixed controller

Human and AI controllers may command members of the same or opposing factions
through identical authoritative command eligibility.

## First-demo rule decisions

- 1–4 humans.
- Pre-match fixed factions.
- One finite resource economy.
- No simulation pause for browser focus/menu loss.
- Reconnect grace with safe-carpet behavior and visible state.
- Late join only through an explicit preset and checkpoint policy.
- Results include objective, faction/member contribution, duration,
  disconnect/AI-takeover facts and compatibility tuple.
- 30-minute infrastructure cutoff is not silently represented as the faithful
  victory condition.
- Voice chat, accounts, ranked matchmaking and mutable alliances are deferred.

## Join journey

```text
Open link
-> browser/GPU/audio/storage/network preflight
-> room identity and ephemeral member
-> content/rules/build compatibility
-> lobby and faction/controller assignment
-> load and readiness
-> safe spawn
-> match
-> reconnect/repair if required
-> result
-> rematch/new room
```

Every failure names the failed requirement and remedy: unsupported
browser/GPU, old build, content/rules mismatch, full/closed room, server
unavailable, corrupt cache, load timeout, rejected command or reconnect expiry.

The exact focus, suspension, controller-lease, safe-carpet and reconnect state
machines are governed by
[`browser-lifecycle-and-reconnect-contract.md`](browser-lifecycle-and-reconnect-contract.md).

## Networking is more than positions

Bandwidth remains bounded by replicating commands, events, sparse deltas and
relevance-filtered snapshots. Required state includes:

- movement and correction;
- ability lifecycle and impacts;
- resource custody;
- actors/AI;
- objectives/results;
- terrain/structure operations;
- hazards/weather phases where authoritative;
- checkpoint identity and hashes.

Meshes, particles, dust, foliage motion, most audio and small debris remain
local presentation.

## Abuse and privacy floor

- Authenticated opaque room/member tokens.
- Command/rate/allocation limits and malformed-message rejection.
- No client-supplied result or arbitrary asset URL.
- Bounded chat/pings if later added.
- Minimal operational logging; no owned Track P identifiers/data.
- Redacted failure bundles and explicit retention.
- Room removal/ban and service-level takedown path before public hosting.

## Acceptance scenarios

1. Two humans cooperate against AI and conserve shared-resource invariants.
2. Two humans fight adversarially with authoritative impacts/world edits.
3. Human and AI share/oppose factions through the same controller interface.
4. Client disconnects, safe state begins, reconnect checkpoint/tail repairs,
   and no duplicate resource/result occurs.
5. Malformed, stale, excessive and unauthorized commands are rejected.
6. Four Chrome clients plus Linux authority complete match and rematch with
   replay/hash evidence.

## Track P separation

Track P may separately document original MC2 network setup, semantics, timing
and remc2's current LAN transport. No source, packet layout, recording, fixture
or legacy protocol code transfers into this contract or implementation.
