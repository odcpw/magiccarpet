# Multiplayer rules, player journey, and session identity

> Historical decision: superseded on 2026-07-23 by the split preservation and
> original-successor contracts. MC2 network behavior remains private research;
> the commercial game's authoritative rules are original product design.

Version: 0.2  
Status: accepted MC2-module session baseline  
Tracking bead: `mc-655`  
Depends on: product promise v0.4, portability baseline v0.2  
Decision date: 2026-07-23

## Decision

Every first-demo composition is one **Island Session** running one
server-authoritative simulation. Solo, cooperative, free-for-all, fixed-team,
and mixed human/AI play differ only through immutable lobby assignments and a
versioned rules-and-objective profile. They do not get separate entity models,
commands, economy, physics, terrain, AI, replay, save, or network stacks.

The invitation default is **Cooperative Expedition**: one to four human
wizards in one faction, neutral ecology, and one AI rival faction. The exact
playable-demo proof set remains:

1. two humans in one faction against one AI rival;
2. two humans in separate factions against each other;
3. one human and one allied AI in one faction against one AI rival.

The architecture also expresses solo, four-way free-for-all, and fixed 2v2.
Those compositions are compatibility obligations, not promises that every
composition will be fully balance-certified for the first shared link.
`mc-ta0` selects a co-op-first, PvP-first, equal, hybrid, retest, or stop
direction from observed post-demo play, rather than architecture guessing the
launch emphasis now.

## Historical boundary

### Accepted reference facts

The reviewed Magic Carpet 2 manual/reference material establishes network play
for two to eight individually identified competitors. Players use the same
session number, choose a wizard/colour slot and preselect one network level;
network play does not follow the campaign's serial narrative progression. It
cannot be paused. The material exposes player names/messages and map/player
information, but it does not establish cooperative teams.

It also does **not** document the network victory predicate, exit activation,
mana quota, friendly fire, death/rebuild/elimination, scripted network
objectives, late joining, reconnect, ties or overtime. The earlier planning
hypothesis “destroy every enemy wizard and then exit” is therefore rejected as
an accepted historical fact until controlled two-instance observation and
private analysis prove or falsify it.

These facts are functional reference evidence. Original multiplayer maps,
level data, prose, menu presentation, network protocol, names, assets, and
balance tables are not implementation inputs or redistribution material.

### Deliberate modernization

- Humans may share a fixed faction.
- A faction may contain human and AI members.
- Cooperative Expedition uses a short authored objective chain before the
  rival-defeat-and-exit climax.
- The provisional adversarial `BanishAndExit` profile is our testable demo
  rule, not a claim about original network victory.
- A secure browser room, reconnect, rematch, accessibility, and failure
  recovery journey replaces period modem/LAN setup.
- A server is the sole authority; the room creator is never a listen-server
  host.
- Team outcome is primary while individual contribution remains visible.

### Still evidence-driven

Owned-game observation and approved private analysis may refine Fireball
self-damage, resurrection, banishment, exit activation, spell targeting,
carrier access, exact stronghold behavior, and the entire original network
victory/loss lifecycle. Such a finding changes a versioned rules profile or
creates a fidelity-ledger entry. It must not create a second simulation path or
silently alter this session model.

The faithful MC2 multiplayer profile and every modern cooperative/team profile
are module data over that one session model. Historical transport compatibility
is not a goal.

The exact registered GOG build needs bounded follow-up evidence: a repeatable
two-instance network harness; source/target damage matrix; death/rebuild
lifecycle; independently varied victory/exit predicates; network-level
inventory; then Ghidra cross-checks of relationship, damage, death, win, exit
and objective-loading behavior. Only fresh behavioral contracts and synthetic
vectors cross into implementation.

Public search corroborates the 2–8-player setup, wizard/colour slots and
single-level selection in the [manual
transcription](https://www.abandonwaredos.com/docawd.php?idg=2966&sf=magiccarpet2readme.txt&sg=Magic+Carpet+2%3A+The+Netherworlds&st=manual);
it supplies no reliable missing victory rule. The owned reference card is the
primary source for the network setup/control facts
(`OWN-MC2-REFCARD-200006`), read with the owned manual
(`OWN-MC2-MANUAL-200006`).

## Authoritative identity model

### Stable identities

| Identity | Lifetime | Owns or selects | Never means |
|---|---|---|---|
| `RoomId` | Lobby through post-match/rematch choice | Invite admission, lobby membership, selected map/preset | Network authority, a player account, permanent social group |
| `ParticipantId` | One guest's presence in one room | Ephemeral display label, coordinator eligibility, permitted lobby preferences, mapping into each new match | A persistent account, gameplay entity, faction or reconnect authority |
| `MatchId` | One countdown through one terminal result | Seed, participants, immutable profile/content/protocol IDs, event log | A rematch or save slot |
| `FactionId` | One match | Objective progress, relationship matrix, shared permissions, shared information eligibility | A mutable alliance or individual wallet |
| `MemberId` | One reserved match slot | Wizard, controller kind, personal resources, attribution, stronghold/carrier | Browser connection or display name |
| `ConnectionId` | One transport attachment | Authenticated command stream for exactly one member | A second member, ownership, or life |
| `EntityId` | One authoritative entity lifetime | Ordered state for a wizard, carrier, stronghold, creature, projectile, or objective | A client-created authoritative object |
| `RulesProfileId` | Immutable build/content contract | Relationships, objectives, limits, recovery and results flags | Arbitrary host-side scripting |
| `EngineApiVersion` | Build admission through artifact retirement | Exact game-module API and deterministic host contract | A compatible-looking Unity version |
| `SimulationBuildHash` | One immutable client/server build family | Exact authoritative simulation implementation | A presentation bundle or marketing version |
| `ProtocolVersion` | Connection admission through disconnect | Wire schema and command/snapshot compatibility | A best-effort negotiation hint |
| `GameModuleSetId` | Lobby admission through immutable result | Ordered module IDs, versions, schemas and dependency graph | A marketing title or permission to redistribute content |
| `ContentRoot` | One exact module-set build | Content-addressed public package graph required by the room | Proof of ownership or a download entitlement |
| `RulesHash` | One exact profile revision | Canonical authoritative rule data and bounded behavior graphs | Client-local presentation preferences |
| `OwnedDataCompatibilityRoot` | Optional, one approved private-overlay session | Equality of independently imported local owner-data normalization | Ownership proof, payload identity exposed to peers, or permission to serve it |

A member is the durable identity inside one match. Reloading the page or
replacing a connection reattaches to the same member; it cannot create another
wizard, stronghold, carrier, spell inventory, score line, or mana balance. A
`ParticipantId` safely carries a consenting guest across Results and
RematchLobby. A rematch creates a new `MatchId`, new member/entity identities,
a new seed, and clean authoritative state. Its carryover whitelist is only
participant/display label, coordinator eligibility, allowed lobby preferences
and local client settings; readiness is false. No prior member, faction,
entity, economy, objective, RNG, command sequence or result state carries.

### Module-set admission and mismatch recovery

- The canonical immutable session compatibility tuple is
  `EngineApiVersion + SimulationBuildHash + ProtocolVersion +
  GameModuleSetId + ContentRoot + RulesProfileId + RulesHash`, plus
  `OwnedDataCompatibilityRoot` only when a separately approved local overlay
  is enabled. The server chooses and allowlists one exact tuple before opening
  the room.
- Lobby discovery exposes required public package identities, sizes,
  provenance, and compatibility—not private original payload or analyzer data.
- A client validates dependencies, schemas, hashes, capabilities, revocation,
  and local budgets before admission. Missing public packages require explicit
  acquisition consent; corrupt, unavailable, cyclic, revoked, oversized, or
  incompatible sets fail closed with a precise remedy.
- No participant may enter simulation with a merely similar module version or
  silently substituted rules profile.
- Room admission, server snapshots, saves, checkpoints, replays, results, and
  diagnostics bind to the exact tuple. Migration is explicit and
  deterministic.
- An optional owner-supplied MC2 data overlay, if separately approved, is
  imported independently by every client and server operator. A compatibility
  root proves equality only; the room, CDN, host, and peers never deliver that
  payload.

### Membership and relationships

- The playable demo has four fixed `FactionId` slots.
- The demo profile caps active humans at four. Identity/schema widths, command
  encoding and deterministic collections must not hard-code a maximum below
  the documented eight original competitors; promoting more than four active
  humans/factions requires a separate post-demo performance, UX, balance and
  networking gate.
- A member has exactly one faction for the entire match.
- A faction has zero or more human or AI members as allowed by its profile.
- Lobby start freezes member/controller/faction assignment. No command can
  transfer a member between factions after countdown begins.
- Empty factions stay disabled. They are never silently populated.
- `SameFaction`, `OpposingFaction`, `Neutral`, and `Self` are the complete
  relationship categories used by authoritative targeting and damage.
- Every member has a stable, unique visual identity in the match. Colour is
  never the sole cue: name/number, shape or icon and accessible map/HUD
  treatment must disambiguate it.
- Mid-match player alliances, betrayal, diplomacy, voting, and pacts are
  excluded. `mc-km2i` is the only gate that may reconsider temporary Pacts.
- Temporary creature control is an entity status owned by a member and
  attributed to that member's faction. It is not a change in player
  relationship state.
- AI submits the same authoritative gameplay commands, pays the same declared
  costs, obeys the same relationships, and receives only its faction-eligible
  information. Any researched rival asymmetry must be separately declared,
  visible in a profile, and covered by replay tests.
- AI difficulty may change planning, reaction delay, aim error, aggression and
  retreat thresholds. It may not secretly change damage, health, mana,
  relationship, vision, ownership or command legality.

### Player, faction, and world ownership

| State | Authority and ownership |
|---|---|
| Wizard health, life/recovery state, casting readiness, spell loadout and upgrade state | Member |
| Kill, assist, objective, economy, terrain and reconnect attribution | Member, aggregated for faction results |
| Primary stronghold and carrier | Member; allied interaction is permissioned, never implicit transfer |
| Objective progress, victory eligibility, shared detection and team permissions | Faction |
| Loose mana, neutral ecology, settlements, hazards, weather, terrain, exits | World/authority |
| Room selection, ready state and lobby coordinator | Session service; coordinator has convenience privileges only |
| Simulation, RNG, ordering, validation, damage, economy, objectives and results | Authoritative server |

The room creator may initially coordinate lobby choices, but owns no gameplay
authority. Whenever the current coordinator becomes ineligible during
`OpenLobby`, `LaunchLocked`, `Results`, or `RematchLobby`, the server selects
the lowest-ordered currently connected human `ParticipantId`. Reelection may
happen repeatedly, never mutates match state, and a returning former
coordinator does not automatically reclaim the role. AI can never coordinate
a room. During `InMatch` the role is dormant; before a result/rematch action is
accepted, the same rule guarantees one eligible coordinator or reports that no
human remains.

## Conserved mana and allied permissions

World mana is a closed authoritative ledger across declared states such as
bound-to-entity, loose, possessed, carried, secured, committed to a persistent
world object, and spent/released by an accepted rule. Every transition has one
source, one destination, one ordered event, and one member/faction attribution
where applicable.

- A member owns possessed mana, that member's carrier reserve, and secured mana
  in that member's stronghold.
- Faction objective progress may sum members' secured mana without moving,
  duplicating, or creating it.
- Same-faction members do not share a silent spendable wallet.
- Possession, reconnect, rematch, AI takeover, objective aggregation,
  resurrection, abandonment, and faction results cannot mint mana.
- Direct member-to-member mana trading is excluded from the demo. Mana changes
  owner only through an explicit world mechanic accepted by the economy
  contract.
- Possession claims neutral or exposed opposing mana for the casting member.
  It cannot overwrite same-faction ownership. Carrier cargo and secured mana
  cannot be possessed until an authoritative loss/destruction rule exposes it.
- Only the owning member's carrier may reserve and collect that member's
  claimed batches.
- A carrier deposits to its owning member's stronghold. An ally cannot withdraw
  from, redirect, dismiss, or spend another member's carrier or reserve.
- Destroying a carrier re-releases its cargo at the loss location without
  changing global total. Stronghold damage/destruction moves an explicitly
  declared quantity from secured to exposed/released state; it cannot erase or
  create mana.
- A member may physically shelter in and defend an allied stronghold. If the
  member's own stronghold is unavailable, an active allied stronghold may be a
  fallback respawn anchor without spending or transferring the ally's mana.
- An ally cannot bank, withdraw, refill, route a carrier, build, grow, repair,
  demolish, reconstruct, or invoke owner controls at that stronghold.
- The demo settlement is a neutral, non-ownable landmark. There is no hidden
  allied settlement economy or permission system.

`mc-lljn` must implement these rules as typed ledger transitions and prove
global and per-owner conservation through join, death, reconnect, abandonment,
recovery, victory, timeout, and rematch.

The derived objective invariant is:

`FactionSecuredMana(f) = sum(SecuredMana(member)) for members in f`

`FactionSecuredMana` is never writable and is never a second wallet. A mana
batch and member-owned structure/carrier store their member owner; faction is
derived from that immutable member assignment.

## Damage, targeting, and shared terrain

The demo baseline uses one relationship filter at authoritative effect
resolution. Visual collision, aim assist, client prediction, particles, or
local audio cannot change the filter result.

### Same-faction policy

- Direct wizard, projectile, controlled-creature, and member-owned area-effect
  health damage, impulse, debuff, steal and capture effects on same-faction
  members are disabled.
- Offensive homing and target assistance cannot acquire a same-faction wizard,
  carrier, or stronghold.
- Explicitly beneficial or neutral interactions may target same-faction
  entities where their spell contract allows it.
- Self-damage from the selected demo spells is disabled in the baseline demo
  profile. A measured faithful profile may enable a confirmed behavior without
  changing the relationship implementation.
- An allied target rejection or harmless allied collision still consumes the
  ordinary accepted cast commitment; it cannot become a free authority probe
  or rate-limit bypass.
- Neutral ecology and environmental hazards remain dangerous to every faction.
  Faction membership is not immunity to lava, storms, volcanoes, water danger,
  neutral creatures, collision, or map-authored hazards.

### Opposing-faction policy

- Opposing wizards and their eligible entities may damage and target each
  other according to the spell/entity contract.
- No PvP profile can obtain same-faction immunity by changing factions after
  start, because membership is immutable.
- A controlled creature uses its controller's faction relationship for
  authored attacks; it remains the same entity and does not rewrite the
  relationship graph.

### Terrain policy

Terrain is one shared world, not faction-private geometry:

- an accepted terrain edit is visible, collidable, replayed, and relevant to
  all participants;
- same-faction direct health-damage immunity does not undo a crater, restore a
  blocked path, create a private bridge, or hide changed cover;
- objective-critical volumes, spawn safety, stronghold foundations, map bounds,
  and minimum playable terrain constraints use profile-declared edit guards;
- legal terrain edits may still help or inconvenience allies, so the HUD warns
  on a protected/allied-critical target and the event log attributes the edit;
- environmental consequences triggered through a legal edit use ordinary
  world-hazard rules rather than secretly reversing the edit for allies.

This preserves the joy and strategic consequence of one deformable world.
Anti-grief limits, protected volumes, cast budgets, and moderation belong to
the terrain, UX, and threat-model beads; they may not introduce divergent
terrain truth.

## Information and map policy

The authority computes information eligibility before presentation.

- A member always receives that member's own exact state and the faction's
  objective state.
- Same-faction members share exact allied wizard, stronghold, carrier, exit,
  and faction-objective status.
- Static world discoveries and accepted player pings are faction-shared.
- Live enemy and neutral-actor detection is the union of current
  faction-eligible sensors. Lost actors become timestamped, decaying last-known
  contacts rather than exact live tracks.
- A discovered enemy stronghold may remain last-known, but current
  damage/destruction state updates only while faction-observed. Opponent bank
  detail is hidden unless an objective deliberately publishes an aggregate.
- Loose mana, creatures and terrain mutations are current only when
  faction-observed; static terrain, public objective sites, phase, weather and
  exit information declared by the briefing are public.
- Opposing factions never receive an exact hidden position merely because
  another faction or the server knows it.
- AI receives the same eligible sensor snapshot as a human member of its
  faction. Debug omniscience is unavailable in a release profile.
- Eliminated players may observe only faction-eligible views until the result
  is terminal. They may cycle allied views but cannot ping, select hidden
  targets or issue commands. Cross-faction free camera, if ever added, begins
  only after the match can no longer be influenced.
- Results may reveal full-map and opposing statistics only after terminal
  authority has frozen the result.

The HUD/map contract owns presentation and accessibility; the wire contract
owns redaction and schema. A hidden entity must not appear in ordinary client
state and merely rely on the renderer not to draw it.

## Presets and rule matrix

All presets instantiate the same identities, commands, systems, relationship
filter, event ledger, snapshots, and result record.

| Rule | Solo Expedition | Cooperative Expedition | FFA | Fixed 2v2 | Mixed proof |
|---|---|---|---|---|---|
| Human assignment | One human in Faction A | 1–4 humans in Faction A | One human per enabled faction | Two humans per faction | One human in Faction A |
| AI wizard assignment | One rival in Faction B | One rival in Faction B | None by default; explicit lobby fill only | Optional pre-start fill for an empty declared seat | One ally in A, one rival in B |
| Neutral ecology | Enabled | Enabled | Enabled | Enabled | Enabled |
| Economy | Individual possession/carrier/stronghold; faction sum | Same | Same | Same | Same |
| Same-faction direct damage | Disabled | Disabled | Not applicable within one-member factions | Disabled | Disabled |
| Terrain/world hazards | Shared and universal | Shared and universal | Shared and universal | Shared and universal | Shared and universal |
| Information | Own faction | Faction-shared | Own faction only | Faction-shared | Faction-shared |
| Objective | Authored chain, banish rival, exit | Shared authored chain, banish rival, exit | Banish every opposing faction, exit | Banish opposing faction, exit | Shared chain, banish rival, exit |
| Team result | One-member faction | One faction result plus contributions | One-member faction | One faction result plus contributions | One faction result plus contributions |
| Demo balance gate | Flight/loop baseline | Required two-human proof | Required two-human duel profile only | Compatibility proof, not balance gate | Required human/AI-faction proof |

The proof identifiers are:

- `Proof.Coop2vAI1`: `H1 + H2 @ A` versus `AI1 @ B`;
- `Proof.DuelH1vH1`: `H1 @ A` versus `H2 @ B`;
- `Proof.MixedH1AI1vAI1`: `H1 + AI1 @ A` versus `AI2 @ B`.

The default 1–4-player invitation varies only the human membership of Faction A
in Cooperative Expedition. It never adds another rival merely because a
faction slot is empty.

## Objectives, victory, timeout, and results

### Objective authority

- Objective nodes and transitions are ordered authoritative state.
- A faction may have shared progress while the event log retains contributing
  members.
- No client, lobby coordinator, room owner, score total, or local cutscene can
  declare victory.
- The Expedition profile teaches flight/economy/stronghold/world interaction
  through a compact sequential chain, then requires rival banishment and an
  explicit exit.
- Adversarial profiles require permanent elimination/banishment of all enabled
  opposing factions before the winning faction's exit becomes usable.
- A faction is not eliminated merely because one member dies. It is eliminated
  only when every member is permanently unable to return under the active
  recovery rules.

When an adversarial exit is unlocked, the first active member of the sole
eligible faction to complete the authoritative exit interaction ends the
match for that faction. The entire faction wins; the exiting member receives
contribution credit, not a separate personal victory.

### Death, recovery, and spectating

- Each member has an individual primary stronghold and carrier.
- Ordinary death follows the same profile-declared stronghold resurrection or
  no-stronghold emergency-recovery rules in every preset.
- Recovery follows one state family:
  `Active → DeadPending → AnchoredRespawn | EmergencyRecovery → Active`,
  otherwise `Eliminated → Spectating`.
- If a member's primary stronghold is unavailable, an active allied stronghold
  is the fallback anchor. If no faction stronghold is available, each member
  has one bounded emergency recovery per match at a profile-declared safe
  faction location.
- Emergency recovery restores only the minimum non-transferable capability
  needed to attempt rebuilding; it creates no world mana or objective
  progress. A later death without an eligible anchor after that recovery is
  consumed permanently eliminates the member.
- There is no downed/revive, lives purchase, buyback or spawn-wave system in
  the demo.
- A recovering member remains a faction member and cannot issue normal
  movement/cast commands until authority returns control.
- Permanent banishment removes control and enters the own-faction spectator
  policy.
- A member's banishment cannot delete or duplicate the faction's ledger.
  World/economy contracts explicitly define what remains, releases, or becomes
  unavailable.
- The exact resurrection cost, timing, emergency-rebuild opportunity, and
  stronghold destruction semantics are frozen by the loop/fidelity contracts,
  then referenced by `RulesProfileId`.

### Timeout and tie

The design target is 20 minutes, the accepted first-test range is 15–25, and
the infrastructure cutoff is 30 minutes including a short visible overtime
stage. The cutoff is not a faithful victory condition:

- the server enters a declared terminal `TimeoutIncomplete` result if no
  faction completed its required exit;
- score, kills, surviving health, or secured mana do not silently manufacture
  a winner;
- the result still records objective depth, pressure, eliminations, economy,
  and why the match failed to resolve;
- post-demo tests may propose a faster competitive profile, but it must have a
  new profile ID and explicit victory rule.

A simultaneous terminal event is resolved by canonical event ordering, not
wall time or packet arrival. All accepted same-tick commands, effects, deaths
and objective transitions resolve before victory evaluation. Mutual
elimination is an explicit draw; if the accepted ordered events otherwise
define no eligible winner, the result is an explicit draw/incomplete category.

### Authoritative result record

Every result contains:

- terminal outcome and reason, winning/eligible factions, objective history,
  elapsed ticks, seed, build/content/protocol/rules IDs, and replay hash;
- faction totals and member contributions;
- wizard eliminations, deaths, recoveries, assists and abandonment;
- mana released, claimed, lost, carried, secured, committed, spent and
  remaining, with conservation reconciliation;
- stronghold/carrier creation, damage, destruction and recovery events;
- objective contributions, exits, creature interactions, and attributed
  terrain edits or allied-critical warnings;
- connection interruptions, successful recoveries, permanent disconnects, and
  latency quality bands suitable for interpreting the test.

Exact assist windows and presentation belong downstream, but one ordered event
may declare one primary source and a deterministic ordered set of contributors.
Individual statistics never change victory, grant persistent power, or create
a public ranking in the demo.

## Join, reconnect, abandonment, and late-join policy

### Before match start

- A valid invite may reserve one available human seat until the lobby starts.
- The lobby exposes preset, map/content version, faction assignment, member
  types, region/server, privacy, readiness and minimum requirements.
- Every profile slot is visibly `Human`, `Open`, `AI`, `Reserved reconnect`, or
  `Closed`; an empty slot never silently becomes an AI or rival.
- Membership can change only while the lobby is open. Assignment changes clear
  the affected ready states.
- Start requires all human members ready, required profile seats filled, every
  member compatible, and server content/build admission accepted.
- The coordinator can select among allowed values and request start; the server
  validates and freezes the match contract.

### After match start

- The demo admits no genuinely new participant after countdown starts. This is
  its explicit late-join policy.
- A returning connection may reclaim only its already-reserved `MemberId`
  using a scoped, expiring reconnect credential.
- Reconnect creates no entity or economy state. The server sends a compatible
  snapshot and ordered catch-up; control resumes only after authoritative
  convergence.
- A duplicate valid connection cannot silently seize control. It reports that
  the member is active elsewhere and requires an explicit **Take control
  here** action; successful takeover rotates the credential, fences the older
  connection, and still exposes only one command source. Exact transport
  fencing belongs to `mc-x8i` and the threat model.
- The simulation does not pause for browser menus, focus loss, hidden tabs,
  reload, sleep, or network loss, including in the solo server-backed profile.
- A disconnected member enters the separately specified authoritative
  safe-carpet lifecycle. Stale input is cleared, and local prediction cannot
  progress damage, economy, objectives, or victory.

### Permanent abandonment

- Reconnect grace is finite and profile-declared; `mc-x8i` freezes the exact
  duration and transition details.
- Choosing **Leave match** is a confirmed immediate abandonment with no
  reconnect grace. Closing/reloading the browser is not the same action.
- On grace expiry, authority permanently disables the member controller and
  applies the same declared abandonment transition on every client.
- An abandoned human is not replaced by AI in the demo.
- In adversarial play, abandonment cannot protect a wizard indefinitely or
  prevent elimination; after the lifecycle transition, ordinary permanent
  banishment rules can eliminate the member/faction.
- In cooperative play, remaining faction members may continue. Existing
  stronghold, carrier, secured mana, and world state are resolved by the
  economy/recovery contract rather than donated or deleted ad hoc.
- There is no faction-surrender command in the demo. If all members of a
  faction become permanently banished/abandoned with no eligible AI member,
  ordinary faction elimination applies.
- If no human member in the entire match returns before every applicable grace
  period expires, the service terminates the unobserved match with an explicit
  `AbandonedNoHumans` operational result. It does not award an AI or score
  victory.

## Browser player journey

### Room state spine

The session service exposes one ordered state spine:

`OpenLobby → LaunchLocked → LoadingBriefing → InMatch → Results → RematchLobby`

`Expired`, `Full`, `BuildIncompatible`, `Maintenance`, and `Closed` are explicit
availability results, not generic errors disguised as room states. Start
creates `LaunchLocked`; backing out requires a declared launch cancellation,
not partial membership edits. A rematch returns participants to an unready
lobby and never carries ready state forward.

### Happy path

1. **Invite:** a player opens an HTTPS URL whose fragment, or an equivalent
   non-request transport, carries a short-lived high-entropy room admission
   capability. The initial HTTP request therefore contains no secret. The host
   sends `Referrer-Policy: no-referrer`; bootstrap exchanges the capability
   once, clears it from visible address/history state, and issues bounded
   participant/reconnect credentials. A human-entered short code, if offered,
   is a rate-limited alias rather than the underlying secret.
2. **Landing and preflight:** the page identifies the game/build, runs
   WebGL2/Wasm/GPU/memory/network capability checks, reports download size, and
   offers a clear compatible/unsupported result before claiming a seat.
3. **Enter:** an explicit user action activates audio and proceeds toward
   the room. Audio failure offers **Continue muted** because every essential
   cue has a visual/text alternative. Pointer lock/fullscreen waits for the
   later **Take flight** action and never traps the player.
4. **Identity:** the player selects an ephemeral display name and accessibility
   defaults. No account, email, social login, or install is required.
5. **Lobby:** players see the selected preset, map, faction/member slots,
   human/AI identity, privacy, compatibility, download state, ready state and
   coordinator. Cooperative Expedition is the default.
6. **Briefing:** the client presents the objective, controls, team identity,
   estimated length, exit condition and any relevant world/mana rule while all
   clients complete deterministic content admission. Loading reports truthful
   named phases such as rules, world, presentation and snapshot rather than
   fabricated percentages; a bounded launch timeout delegates a missing member
   to the ordinary disconnected-seat policy.
7. **Countdown and flight:** server freezes membership/profile/seed, creates a
   new `MatchId`, confirms each member's spawn, accepts **Take flight** as the
   pointer-lock gesture, and releases control on one authoritative tick.
8. **Interruption:** focus/pointer/audio/network state is visible. A recoverable
   interruption offers one reconnect path and never implies that the
   authoritative world paused.
9. **Result:** server freezes the terminal record; the client shows team
   outcome first, personal contribution second, incomplete/failure cause where
   applicable, and a replay/support identifier.
10. **Rematch:** present members may ready for the same preset/map with a new
    seed, change allowed lobby choices, leave, or create/share a new room. A
    rematch is clean state, not a simulation reset hidden inside the old match.

The returning-player target remains under 60 seconds from navigation to
authoritative flight on the accepted reference connection and cached build.

### Recoverable admission states

| State | Player-facing truth | Primary action |
|---|---|---|
| Invalid invite | Code is malformed or not recognized | Re-enter code or request a new link |
| Expired invite/room | Admission window or room ended | Ask inviter to create/share a room |
| Full room | Human seat cap is reached | Retry only if notified of a vacancy or create a room |
| Match already started | New late join and live spectating are disabled; no match/faction state is exposed | Keep the page open for minimal room-availability polling and claim an open participant seat only if this room reaches `RematchLobby`, or create/request another room |
| Incompatible build/content/protocol | Client and room cannot safely simulate together | Reload/update to the offered compatible build or leave |
| Unsupported browser/GPU | Required capability test failed | Show exact failed requirement and supported-system guidance |
| Download/cache failure | Required content is unavailable or corrupt | Retry verified download, clear only game cache, or report failure |
| Server unavailable/maintenance | Room service cannot admit or run the match | Preserve invite locally, retry with bounded backoff, show status |
| Connection lost in lobby | No match state yet; seat may have a short reservation | Reconnect to lobby or release the reservation visibly |
| Connection lost in match | Server simulation continues and member slot is reserved during grace | Reconnect to the same member |
| Reconnect expired | Member entered permanent abandonment | Observe allowed result/faction view or return to lobby |
| Room result expired | Post-match room no longer exists | Create/share a new room |

Every error names what happened, whether a seat/member still exists, whether
the match continues, the safest next action, and a copyable diagnostic
reference containing no invite/reconnect secret. Infinite spinners, silent
reload loops, raw transport errors, false “paused” language, and ambiguous
ownership are acceptance failures.

## Session privacy and abuse floor

- Demo rooms are unlisted and admission-code based. There is no public room
  directory, stranger matchmaking, public profile, friend graph, persistent
  identity, ranked ladder, text chat, voice chat, spectator join, or
  user-generated content.
- A room code grants only bounded admission while the lobby accepts it. It does
  not grant administration, arbitrary member control, server authority, asset
  access, or permanent reconnect rights.
- Display names are ephemeral, length/character bounded, normalized, escaped,
  and filterable. The server also assigns an unambiguous short member label.
- Create/join/ready/start/command/reconnect/rematch operations are
  authenticated, schema validated, sequence checked and rate limited.
- Reconnect credentials are member-, room-, match-, build-, and expiry-scoped,
  rotated on successful use, and never placed in logs, replay files, analytics,
  screenshots, browser-visible history/address state, referrer data, or
  shareable URLs.
- Invite capabilities and participant credentials follow the same log,
  diagnostic, screenshot, analytics, referrer and browser-history redaction
  rule. The initial request cannot contain the invite capability.
- The server returns only faction-eligible state and minimal admission
  information. Client secrecy is not an anti-cheat control.
- The demo collects no account graph and no public leaderboard. Any test
  telemetry is versioned, minimized, documented, and separated from the
  authoritative result/replay evidence.

`mc-vgo` owns the full threat model, denial-of-service budgets, deployment
secrets, moderation/support flow, retention and incident response. Those may
tighten admission but cannot turn the room creator or browser into authority.

## Canonical acceptance scenarios

Each scenario uses synthetic/public-safe content, fixed seed and command
stream, exact build/content/protocol/rules IDs, a terminal result, event-ledger
reconciliation, and matching authoritative hashes through server, required
native conformance client, and Chrome client.

### A. Cooperative proof

`H1 + H2 @ A` and `AI1 @ B` start from the lobby. H1 and H2 each receive one
wizard, stronghold and carrier. They share objective/detection state but not
spendable mana. Same-faction Fireballs cause no direct health damage; H1's
legal Crater changes the world for both. They complete the chain, permanently
banish AI1, reach the exit and receive one faction victory plus ordered
personal contributions.

### B. Adversarial proof

`H1 @ A` and `H2 @ B` use identical spells, economy, recovery, terrain,
commands and wire state. Opposing damage is enabled and information is
redacted per faction. Once one faction is permanently eliminated, the
remaining faction's exit becomes eligible. Score or mana cannot substitute for
the exit.

### C. Mixed-controller proof

`H1 + AI1 @ A` and `AI2 @ B` prove that human and AI members may share a
faction without a special economy or information channel. AI1 has its own
member-owned stronghold/carrier/mana and consumes only Faction A's eligible
state. The same replay can replace AI1's command source with a recorded human
stream without changing command semantics.

### D. Reconnect conservation proof

A human disconnects while carrying and securing mana and while terrain/objective
events are in flight. The old connection is fenced, safe-carpet policy applies,
the member reattaches, snapshot/catch-up converges, and the ledger/entity count
is unchanged. Repeating, racing, and expiring the credential cannot duplicate
commands, entities, resources, progress, or result attribution.

### E. Timeout and rematch proof

No faction completes its exit before the cutoff. Authority emits
`TimeoutIncomplete`, reconciles the world ledger and refuses to nominate a
score winner. A rematch creates a new match/seed/identities and clean world
while the old immutable result/replay remains retrievable for its declared
retention window.

### Required invariants

- Exactly one authoritative member controls each active wizard.
- A member has exactly one immutable faction per match.
- A connection controls at most one member; reconnect never creates a member.
- Human and AI controllers emit the same gameplay command types.
- Presets select data/rules/objectives; no preset-specific simulation fork
  exists.
- Global mana reconciliation equals the declared initial total after every
  accepted transition category, including disconnect and rematch boundaries.
- Faction aggregation cannot be spent twice or withdrawn as a hidden wallet.
- Same-faction direct damage, opponent damage, neutral hazard damage, targeting
  and terrain consequences match the declared relationship matrix.
- A client receives no authoritative hidden opponent state outside its
  information entitlement.
- Only the server produces objective transitions, elimination, exit
  eligibility and terminal results.
- Timeout does not invent a winner.
- A rematch shares no mutable entity, economy, objective, RNG, command-sequence
  or result state with the prior match.
- Every rejection/failure identifies the first violated invariant and retains
  enough version/seed/replay/log evidence to reproduce it.
- Landing, lobby, briefing, interruption, result and rematch remain operable
  with keyboard-only focus, visible focus indication, accessible status
  announcements, and visual/text alternatives for essential audio.

The common gameplay command vocabulary is movement/aim, cast, Possession,
stronghold action, carrier action, objective interaction, confirmed
leave/surrender and reconnect. Every command is authenticated to a
server-owned member slot; the server derives faction and relationship.
Clients and AI never submit a trusted faction, ownership result, score or
objective delta.

## Downstream hand-off

| Bead | Must consume from this contract |
|---|---|
| `mc-ahm` | Stable identity model, fixed lobby assignment, controller parity, proof compositions |
| `mc-lljn` | Individual economy ownership, faction aggregation, allied permissions, conservation tests |
| `mc-5nc` | Identity/version schemas, information redaction, reconnect fencing, result/event requirements |
| `mc-9t3` | Faction identity, shared/detected information, allied warnings, objective and results hierarchy |
| `mc-xli`, `mc-13t` | Preset objectives, rival/exit condition, individual stronghold/carrier assumptions |
| `mc-x8i` | No-pause policy, reconnect-only late admission, abandonment boundary, duplicate connection rule |
| `mc-9od` | Full invite-to-rematch journey, coordinator semantics, every admission/recovery state |
| `mc-vgo` | Room/reconnect capability model, no-chat/no-account scope, validation and rate-limit floor |
| `mc-w58.11`, `mc-70q` | Three composition proofs plus reconnect/conservation/timeout/rematch chaos cases |
| `mc-941`, `mc-ta0` | Team/personal outcome evidence used to select launch emphasis without changing core authority |
| `mc-z4a` | Room, match, member and immutable-result lifetimes; no demo account/metagame assumption |
| `mc-km2i` | Pacts remain excluded and must not overload fixed faction membership |

## Explicit non-goals

- No accounts, public matchmaking, public rooms, ranking, progression economy,
  persistent parties, friend graph, chat, voice, drop-in late join, spectator
  admission, replay browser, host migration, listen server, peer authority, or
  user-run dedicated server in the demo.
- No mutable player alliances or separate MC2 Alliance-spell interpretation.
- No shared faction wallet, universal allied ownership, invisible mana
  transfers, private terrain, client-side victory, or AI-only gameplay API.
- No claim that cooperative factions or the browser journey reproduce original
  multiplayer.
- No balance certification for every legal 1–4-human arrangement before the
  named proof set passes.
- No exact reconnect timing, tick rate, bandwidth, spell constants,
  resurrection costs, assist window, retention schedule, or visual treatment
  frozen here; their named downstream contracts must choose and test them.
- No implementation, Unity installation, package choice, networking library,
  service vendor, public deployment, or original asset use is authorized by
  this planning decision.

## Gate decision

Accept when independent review confirms:

1. one matrix covers solo, co-op, FFA, fixed 2v2 and mixed play without
   simulation or netcode forks;
2. individual, faction and world ownership cannot imply duplicate mana or
   ambiguous allied control;
3. friendly fire, targeting, terrain, information, recovery, elimination,
   abandonment, victory, timeout and results are implementable without hidden
   policy choices;
4. the three required playable-demo compositions are precise and testable;
5. the browser journey covers invite, preflight, identity, lobby, briefing,
   match, reconnect, result and rematch plus actionable failure states;
6. accounts, public matchmaking, chat/voice, pacts and true late join are
   explicitly out of scope;
7. downstream beads own the remaining numeric, lifecycle, security,
   presentation and fidelity decisions without reopening the identity model.

Closing this gate approves the multiplayer/session contract as a planning
input. It does not approve implementation or assert that any networked build
already exists.
