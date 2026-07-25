# Product promise and scope box

Version: 1.0  
Status: accepted two-track baseline  
Governing charter: [`program-charter.md`](program-charter.md)  
Tracking beads: `mc-znpx`, `mc-dl0r`, `mc-w58`

## Program promise

Preserve and deeply understand Magic Carpet 2 for lawful owners, while
building an original browser-first commercial carpet-flight game that carries
its finest experiential ideas into living islands and transformed cities with
modern graphics, multiplayer, destruction, atmosphere, and later VR.

The preservation build and commercial game are separate deliverables.

## Preservation promise

Track `P` aims to make Oliver's exact owned MC2 build:

- reproducible through pinned remc2/MC2HD native builds;
- testable through deterministic recordings and original/native state
  comparison;
- understandable through the existing source, DOSBox oracle, Ghidra, Binary
  Ninja, manuals, play and research;
- maintainable through portability, regression and documentation work;
- experimentally browser-capable through owner-local data and Wasm; and
- respectful of Tomas Vesely, Tim Hobbs and the wider contributor ecosystem.

It does not promise a public browser link, bundled data, public fork,
commercial reuse, or license clearance. Those require separate evidence and
approval.

## Commercial successor promise

The commercial game is original IP:

> One click puts friends on living magic carpets in a sublime, reactive world.
> They skim forests and surf, dive between towers, contest finite magic,
> protect moving reserves and strongholds, fight creatures and rival
> sorcerers, reshape routes and selected structures, survive storms and
> volcanoes, and finish a dramatic 15–25 minute expedition that immediately
> invites a rematch.

The successor is not sold as MC2, does not require MC2, and does not reproduce
its 25 realms, catalog, code, assets, text, UI, maps, names, or branding.

## Player fantasy

- A carpet with momentum, bank, skim, altitude, braking and expressive risk.
- Extreme speed that remains readable through camera, horizon, sound, HUD and
  tactical-map design.
- A world that visibly and audibly responds to flight, spells, wind, weather,
  fire, water, creatures, vegetation, terrain and structures.
- Finite magical energy that exists in the world, changes custody, must be
  collected and secured, and can be stolen or lost.
- Escalation from exploration and opportunism through fortification and
  conflict to spectacular magical warfare.
- Rival flyers and creatures governed by the same world rather than scripted
  shooting-gallery targets.
- Places worth remembering: original islands first, recognizable transformed
  city districts later.
- Solo, co-op, PvP, teams and mixed human/AI compositions built on one
  authoritative simulation.
- Spatial wind, surf, forest, wildlife, fire, water, thunder and city ambience,
  plus an original adaptive score.

## Successor architectural identity

The successor consists of:

1. deterministic, presentation-independent simulation;
2. an authoritative Linux room/server;
3. a Unity client/editor baseline protected by the measured retain/fallback gate;
4. browser shell, secure transport, cache and lifecycle integration;
5. original game rules/content packages and safe mod schemas;
6. provider-based worlds and bounded destruction;
7. original or properly licensed Blender/procedural assets; and
8. replay, save, divergence, telemetry, accessibility and operations tooling.

Unity is the provisional baseline because of its CLI/MCP/editor, Blender
integration, URP production path, platform support and later OpenXR route.
It remains reversible through the Chrome/server proof in
[`stack-decision.md`](stack-decision.md).

## First successor social demo

### Audience and platform

- A private HTTPS link suitable for friends.
- Current stable 64-bit desktop Chrome on supported Windows and Linux.
- Keyboard/mouse baseline; controller highly desirable.
- Native development clients permitted, but browser is the release gate.
- Linux authoritative server; no peer or room creator is canonical authority.
- No account needed; room code/link and an ephemeral display identity.

### Session envelope

- Nominal 20 minutes; acceptable 15–25 minutes.
- Visible 30-minute infrastructure cutoff including bounded overtime.
- Returning-player link-to-flight target under 60 seconds on the reference
  connection.
- 1–4 humans in the first demo.
- Certified compositions: solo versus AI; two-human co-op versus AI;
  two-human PvP; one mixed human/AI faction run.
- Results, rematch and new room immediately available.

### Original demo world

- One bounded island/archipelago approximately 3–4 km across.
- Ocean, shoreline, meadow, forest, mountains, caldera/volcano, settlement or
  fortress silhouettes, storm and day/dusk.
- A canonical deterministic surface provider and bounded local structural or
  terrain edits.
- No arbitrary full-world voxel requirement.

### Functional journey

1. Open link, activate audio, preflight and join.
2. Learn speed, bank, climb/dive, strafe/brake, skim, aim, HUD and minimap.
3. Use one original direct projectile spell to release magical energy.
4. Claim and transport exposed energy.
5. Establish or activate a first stronghold.
6. Protect a moving reserve/carrier and bank energy.
7. Choose between resource, creature, rival and objective pressure.
8. Earn and recognize one ability upgrade.
9. Use one bounded world-edit spell tactically.
10. Survive environmental escalation.
11. Complete the selected objective or defeat the required opposition.
12. Reach explicit results and rematch.

### Feature floor

- Expressive, evidence-tuned carpet movement plus comfort options.
- At least five distinct ability roles across direct attack, claim/resource,
  world edit, defense/mobility and information/control.
- Conserved magical-energy custody.
- One stronghold and moving-reserve family with loss/recovery.
- Health, damage, death and recovery.
- One rival flyer using the authoritative command/rules interfaces.
- Four hostile ecological roles: aerial swarm, ground artillery, resource
  predator and area controller.
- Mountains, water, meadow, trees, volcano/lava, wind, storm, clouds and
  day/dusk.
- Sequential objective, failure, result and rematch.
- HUD, minimap, tactical map, objective recall, selection, captions,
  remapping and reduced-roll options.
- Spatial environmental sound and original adaptive musical sketch.
- Room code, reconnect policy, AI assignment, snapshots/repair, rate limits,
  replay/hash evidence and structured failure artifacts.

The first-session, input, accessibility and comfort requirements are governed
by [`browser-first-ux-contract.md`](browser-first-ux-contract.md).

### Demo non-goals

- MC2 names, realms, campaign, exact spell/creature catalog, original assets or
  compatibility claims.
- Full city streaming or universal building destruction.
- Full voxel caves.
- Ranked matchmaking, store, cosmetics, accounts or live-service economy.
- Eight players, mobile, console or VR certification.
- Final visual quality for every asset.

## City vertical slice

After the island demo, prove one transformed real-city district using
properly licensed building/geographic data and original presentation:

- hierarchical streaming and LOD;
- separate visual, collision, structural and interaction payloads;
- selected destructible structures and persistent sparse deltas;
- server-owned structural/gameplay state;
- client-owned cosmetic dust and small debris;
- fantasy transformation rather than photorealistic copying;
- attribution and derivative-data obligations built into the asset manifest;
- browser low tier and native quality tier.

## Milestones

1. `P0` preservation baseline.
2. `P1` owner-local preservation modernization/Wasm proof.
3. `S0` Unity/browser/server/Blender/agent stack proof.
4. `S1` original island social demo.
5. `S2` original city vertical slice.
6. `S3` commercial production, business and launch-scope decision.
7. `S4` production alpha with the selected original campaign and multiplayer
   content.
8. `S5` 1.0 after performance, accessibility, operations, security, licensing,
   branding, privacy, ratings and support gates.

Track P may continue in parallel and does not block S1. Accepted lesson
packages may improve flight and loop design, but none is required to begin or
complete the successor baseline.

## Scope-change rule

Oliver's approval is required for changes to:

- track separation;
- public use of reconstructed code or original data;
- successor stack;
- browser-first priority;
- authoritative player count or network model;
- first-demo feature floor;
- city/voxel/VR milestone order;
- repository/access boundary; or
- commercial identity and release posture.
