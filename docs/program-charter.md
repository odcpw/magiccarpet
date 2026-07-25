# Magic Carpet program charter

Version: 1.0  
Effective: 2026-07-23  
Status: governing program decision  
Supersedes: the single-product assumptions in product promise v0.4 and the
associated pure-C#/Unity-first, full replacement-MC2 plan  
Tracking epics: `mc-znpx`, `mc-dl0r`, `mc-w58`

## Program purpose

Honor Magic Carpet 2 in two complementary ways:

1. preserve, understand, run, test, and where rights permit modernize the
   original game for lawful owners; and
2. create a beautiful original carpet-flight game that carries forward the
   experience of expressive flight, finite world mana, terrain-changing magic,
   creatures, rival sorcerers, strategic escalation, atmosphere, and social
   play without copying the original game's code or expression.

The two outcomes are related but not the same product, repository, license, or
release decision.

## Track P: private preservation and oracle

Track P uses the exact owned MC2 build and the existing reconstruction
ecosystem:

- `turican0/remc2`;
- `thobbsinteractive/magic-carpet-2-hd`;
- `turican0/dosbox-x-remc2`;
- `turican0/MagicBalls`;
- Ghidra, Binary Ninja, DOSBox, debuggers, captures, manuals, guides, and
  controlled play.

Its goals are:

- reproduce current native remc2/MC2HD builds with Oliver's owned data;
- preserve the deterministic input, internal-state comparison, earliest
  divergence, save/checkpoint, and all-realm regression workflow;
- understand the recovered simulation, flight, terrain, spells, AI, campaign,
  formats, sound, UI, and network behavior;
- identify and document remaining divergences or fragile reconstructed areas;
- determine whether a headless simulation step, stable compatibility API, and
  browser/Wasm owner-data experiment are technically feasible;
- study MagicBalls as a modern-renderer precedent;
- contribute general fixes or documentation upstream when scope, license, and
  provenance permit; and
- preserve an exact, reproducible private reference baseline.

Track P may use original names, owned assets, exact addresses, reconstructed
source, raw state, and original formats inside the private lab. None of those
permissions imply redistribution.

### Track P release states

1. `PRIVATE_RESEARCH`: local builds and evidence only.
2. `PRIVATE_OWNER_BUILD`: Oliver's own native/browser experiment using his
   local data; not shared.
3. `SOURCE_OR_PATCH_CANDIDATE`: only after exact reconstructed-code license and
   lineage review.
4. `OWNER_DATA_PRESERVATION_RELEASE`: only after counsel/rights-holder review,
   no-bundled-data proof, branding decision, source obligations, installer and
   security review, and an explicit release gate.

A browser page is distribution even when it asks the user to supply original
data. Track P does not call itself legally cleared merely because it is
noncommercial or requires ownership.

## Track S: original commercial successor

Track S is a new game and a new codebase.

Its promise:

> Fly a living magic carpet through sublime, reactive worlds—including
> recognizable transformed cities—where momentum, finite magical resources,
> creatures, rivals, weather, terrain and building destruction, strongholds,
> and multiplayer strategy turn every 20-minute expedition into a story.

Track S owns:

- a new product name, world, lore, characters, factions, spells, creatures,
  progression, campaign, objectives, UI language, fonts, icons, art, animation,
  textures, VFX, music, sound, maps, and marketing;
- independently designed simulation and networking code;
- original or properly licensed Blender and procedural asset pipelines;
- browser-first delivery and current desktop Chrome certification;
- authoritative hosted multiplayer supporting solo, co-op, PvP, teams, and AI
  through one simulation foundation;
- later native Windows/Linux and OpenXR paths;
- provider-based heightfield, bounded structural destruction, and later
  volumetric or real-city worlds; and
- a mod/content architecture defined for the successor rather than for MC2.

Track S is inspired by functional game-design ideas and independently measured
feel. It is not an MC2 compatibility layer, a 25-realm reproduction, or a
replacement-expression version of the original campaign.

## Shared bridge

The tracks may share only reviewed, independently expressed material in these
classes:

- general engineering techniques;
- public framework/tool knowledge;
- original project design;
- externally observable mechanics stated at a high level;
- measurements accepted through the provenance protocol;
- synthetic tests and independently chosen vectors;
- deterministic replay/hash/divergence methodology;
- browser performance, lifecycle, input, audio, and networking constraints;
- generic simulation-step, event, presentation, terrain-provider, save, and
  authority contracts; and
- original assets created for Track S.

The tracks do not share:

- reconstructed/decompiled source;
- original addresses, names, memory/state layouts, copied control flow, dumps,
  recordings, or regression images;
- original files, levels, tables, art, audio, text, fonts, UI, maps, branding,
  trade dress, or extracted assets;
- exact bulk constants or catalogs without individual transfer review; or
- MagicBalls/remc2 structures disguised behind renamed wrappers.

Every bridge artifact names source track, destination track, source class,
permitted use, prohibited detail, author, reviewer, and exact hash.

## Technical architecture policy

### Track P

Start from the reconstructed C++ ecosystem because fidelity and preservation
are the purpose. Do not rewrite working recovered behavior merely to satisfy
the successor's architecture.

Evaluate:

1. current remc2 as the canonical reconstructed source;
2. MC2HD for releases, packaging, current community changes, and enhanced
   native presentation;
3. the DOSBox fork as the exact original-executable oracle;
4. MagicBalls as evidence about modern presentation integration;
5. a minimal headless/step boundary;
6. a stable C ABI only if it reduces preservation, testing, or Wasm risk; and
7. Emscripten/Wasm only after native regression is reproduced.

### Track S

Keep authoritative simulation independent from presentation and engine
objects. `docs/stack-decision.md` provisionally selects Unity 6 LTS/C#/URP,
Unity Web, Linux headless authority, Blender LTS/`bpy`, and a small TypeScript
browser shell. The measured proof may retain, remediate, or replace that
baseline.

Unity is the provisional client/editor baseline because it offers:

- CLI and batch automation;
- MCP editor control;
- mature Blender/FBX/GLB import;
- URP, Shader Graph, VFX, animation, profiling, physics tools, UI, audio, and
  asset workflows;
- Windows/Linux/Web targets; and
- OpenXR support for later native VR.

Unity is not automatically authoritative. Authoritative movement, combat,
resources, objectives, world edits, AI decisions, saves, replays, and network
state must remain deterministic and testable without rendering, PhysX result
order, wall-clock time, or editor state.

Browser-native Three.js/WebGPU and a native/Wasm Rust or C++ core are invoked
only if the Unity proof misses a declared hard gate in bootstrap size, memory,
threading, server parity, iteration reliability or city streaming. Godot is a
contingency if both primary paths fail. The decision gate uses the same
synthetic workload and evidence axes for every invoked candidate.

## Repository and access topology

```text
/home/oliver/Projects/odcpw/magiccarpet
    PRIVATE_LAB
    owned data, evidence, upstream reconstruction checkouts,
    preservation builds, source register, planning, private beads

/home/oliver/Projects/odcpw/carpet-flight
    SUCCESSOR_CANDIDATE
    new history, original code/assets only, stack selected later,
    no recursive copies or shared Git/artifact caches

optional future preservation release root
    PRESERVATION_RELEASE_CANDIDATE
    created only after rights/release gate; never the private lab itself
```

Git initialization, first commit, and remote creation still require the
project-specific identity rule. The current private lab is never converted
into either release repository.

## Program milestones

### P0 — Preservation baseline

- pin all upstream revisions and license signals;
- complete the MC2HD clone and submodule inventory;
- reproduce stable/development native builds with owned data;
- run at least one deterministic regression;
- document known exclusions, including current skipped/divergent levels;
- establish private retention and build manifests.

### P1 — Preservation modernization proof

- prove a clean frame/step boundary or document blockers;
- run headless where feasible;
- compare native state hashes;
- prove or falsify a minimal Wasm segment;
- study one MagicBalls state-to-render exchange;
- decide whether an owner-only browser prototype is worth building.

### S0 — Successor stack proof

- test the pinned Unity CLI/Pipeline/MCP, URP, Chrome and Blender workflow;
- record the later native OpenXR package path without installing XR packages
  into S0;
- test deterministic simulation outside presentation;
- run Unity Web against the declared synthetic scene and budgets; invoke a
  browser-native challenger on the same proof only after a hard-gate miss;
- test Linux headless authority and four Chrome clients;
- record bootstrap, memory, frame, asset, network, iteration-time, and agent
  automation evidence;
- select the stack and fallback.

### S1 — First social demo

One original 15–25 minute world, playable from a link:

- expressive carpet flight;
- Fireball-like direct spell under a new public identity;
- claim/transport/secure finite magical energy;
- one stronghold/carrier loop;
- one rival controller and at least four hostile ecological roles;
- mountains, water, meadows, trees, volcano, storm, day/dusk;
- HUD, minimap, objectives, death/recovery, results/rematch;
- spatial environmental sound and original adaptive music;
- solo, two-human co-op versus AI, two-human PvP, and mixed human/AI proof;
- authoritative hosted simulation, reconnect, replay/hash evidence; and
- original/proxy/licensed content with a complete manifest.

### S2 — City vertical slice

- one recognizable but transformed city district generated from properly
  licensed geographic/building data;
- streamed visual tiles and a bounded authoritative interaction layer;
- selected destructible buildings/terrain rather than universal granular
  simulation;
- original factions, objective structure, art direction, audio identity, and
  progression;
- measured browser and native quality tiers.

### S3 — Commercial production decision

Use S1/S2 evidence to choose:

- campaign scale and structure;
- business and source-license model;
- launch emphasis among solo/co-op/PvP;
- content-production budget;
- supported native/VR targets;
- hosting and moderation posture; and
- whether preservation work receives a separately cleared public release.

## Non-goals

- Claiming the reconstructed engine is cleanly MIT, GPL, or commercially
  shippable without resolving the contradictory lineage/license signals.
- Rebuilding all 25 MC2 realms as the commercial successor's campaign.
- Copying MC2 assets, names, story, UI, spell catalog, creatures, levels,
  constants, or reconstructed implementation into Track S.
- Forcing Track P through Track S's engine abstraction.
- Forcing Track S to use remc2 merely because preservation work uses it.
- Choosing Unity, Three.js, Godot, Rust, C++, or a network library without a
  reproducible measured gate.
- Promising fully destructible entire cities, MMO scale, console/mobile parity,
  or VR launch support before their own evidence gates.

## Decision ownership and change control

Oliver owns product direction. Engineering evidence can recommend but cannot
silently merge tracks or broaden release authority.

A change requires a recorded decision when it alters:

- the track boundary;
- source/reconstructed-code access;
- public release posture;
- commercial product identity;
- authoritative simulation stack;
- browser/native/VR targets;
- multiplayer authority;
- city/world representation;
- repository/access topology; or
- first-demo feature floor.

All downstream documents and beads must name `P`, `S`, or `SHARED`. An item
without a track is not implementation-ready.
