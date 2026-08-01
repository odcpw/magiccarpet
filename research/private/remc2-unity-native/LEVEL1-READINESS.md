# Level 1 playable-readiness record

Date: 2026-07-27
Scope: private owner-data preservation build, remc2 source to Rust simulation
to Unity desktop. No DOSBox or original-binary analysis is part of this
milestone.

## Verdict

Level 1 is a playable mission slice, but it is not yet a complete remc2
conformance slice. The ordinary-input desktop
route starts from a fresh session, flies through all five source-mapped
objective predicates and their completion message, collects and casts Fireball,
Possession, Castle, and Speed, fights
Archers and Fireflies, builds castle stage 1, possesses and deposits mana
spheres, fills the castle, enters the authored exit, and reaches result code
10 while alive. The separately tested post-completion group-33/34 multipart
chain is implemented in the simulation and presentation, but the current
ordinary-input completion route does not claim to fight that optional chain.

That is a playable-readiness claim, not a frame-perfect-completion claim.
The registered eight-bit Level 1 mission-simulation fidelity mask is now zero;
it does not cover input, HUD, renderer, other presentation fidelity, or the
newer house-spawned resident action-body register. House action 53 now removes
terrain, allocates exact resident constructors and source slots, and renders
qualified Archer/Villager families, but the four resident AI action bodies
remain explicitly frozen with `dynamic_resident_behavior_gap_mask = 0x0f`.
Every reachable
mission sound on the ordinary-input completion route is tied to its remc2
emitter, listener-relative command, pitch family, play type, and shared
ten-voice arbitration. The effective Level 1 transient palette, objective
minimap marks, and one shared 4:3 DOS-display presentation rectangle are now
native/source-mapped. The Level 1 minimap now uses Rust-owned source-slot
ordering, entity filters, colors, phase rules, sprite anchors, castle rope,
objective marks, and the single-player reveal rules. High-resolution HSPRN
HUD composition is active. The Level 1 objective textbox now preserves exact
source geometry, 33-turn timing, TABLESN selector-61 live indexed-world
blending, HFONT3 text, and HSPRN frame draw order; its explicit
pixel-centre-nearest 320×200-to-640×480 mapping is a presentation adaptation.
Exact sprite hotspots and complete whole-frame source interleaving remain
registered presentation work.
Unsupported presentation is absent or labelled; it is never silently treated
as exact.

## Run it

```bash
cd /home/oliver/Projects/odcpw/magiccarpet/research/private/remc2-unity-native
./run-level1.sh
```

Controls: `W/S` speed, `A/D` strafe, mouse or `I/J/K/L` steering, and `C` or
middle mouse centre. Hold `Control` for the source 2×13 spell menu; left click
chooses the left hand and right click chooses the right hand. Keep that button
held, move across the tier submenu, and release to assign. Outside the menu,
left mouse casts the left-hand spell, right mouse casts the right-hand spell,
and `Space` also casts the left hand. `Esc` releases capture, click recaptures,
`R` restarts, `F11` toggles fullscreen, and `F10` quits.

## Working systems

- Exact-integer player flight, terrain queries, spell activation order,
  camera history, death/input suppression, departure, fade, completion, and
  deterministic native state/event contracts.
- Rust-owned objective switches, pickups, Fireball, Possession, Speed,
  castle placement/construction/teardown, mana spheres and carrier, enemy AI,
  damage/death, group-33/34 activation, the authored exit, and result 10.
- House action 53 uses the qualified BUILD footprints, row-major terrain/RNG
  order, real source allocations, exact Archer/Builder/Villager/Trader
  constructor pairs, and exact initial damage-mailbox dispatch. Their
  subsequent AI bodies remain the explicit resident gap described below.
- Four class-3/model-3 multipart heads, each with a source-slot-linked chain
  of 16 segments, plus six Fireflies and the group-34 scroll are allocated in
  authored order and participate in activation, AI, combat, death, mana
  conversion, rendering, and reset.
- One bounded 1,000-slot source arena owns imported rows, authored actors,
  multipart segments, projectiles, impacts, mana, castle objects, allocation
  fallback, dispatch, and deferred reclaim. Its bookkeeping digest is
  deterministic across replay, reset, and independent sessions.
- Session-local mutable terrain carries all six planes, canonical hashes,
  mutation sequence, reset/isolation guarantees, and an additive native/Unity
  lifecycle bridge.
- Owner Level 1 terrain, sky, palette, HUD inputs, minimap inputs, actor and
  projectile TMAPs, spell icons, effects, music, and sound records are loaded
  through the validated external owner cache.
- The qualified indexed compositor covers terrain, Goats, Archers,
  reset-frozen villagers and dolmens, Fireflies, multipart heads and segments,
  Fireballs, arrows, castle
  projectiles, Possession, spell jars, exit target, impacts, splashes, castle
  root, mana carrier, and mana spheres in source-slot order, with the shared
  reflection-before-primary and terrain-projected shadow passes.
- Five objective textboxes (selectors 48–52) plus the completion textbox
  (selector 53) use exact remc2 layout and source draw order:
  live indexed-world TABLESN blend, HFONT3 text, then HSPRN frame. Their
  foreground and masks remain indexed until the final owner-palette resolve.
- Unity supplies input, audio playback, HUD/window presentation, and the
  desktop shell. It does not advance gameplay state or own gameplay physics.
- Qualified mode contains no `Rigidbody`, collider-driven gameplay,
  diagnostic light, primitive carpet, debug fire sphere, or fallback text
  overlay.
- Owner data stays outside the built player and is supplied through validated
  package, closure, and cache paths.

## Current verification

| Gate | Result |
|---|---|
| Rust workspace | 442 tests in the locked Cargo inventory: atlas 4, C ABI 57, sim core 377, WASM proof 4; exact row bindings resolve against that list |
| Owner cache | 1,249 outputs; deterministic rebuild, sealed PLAN29 identity, accepted native FINAL MAIN source-frame census, and corruption negatives |
| Unity EditMode | 29/29 passed |
| Unity PlayMode | 1/1 passed |
| Built-player route | Stage 5, result 10 at tick 14,720 in the rebuilt desktop player |
| Built-player post-completion route | Stage 5, result 10 at tick 21,039 after group 33, first multipart, and group 34 |
| Native self-test | Five checkpoints, two sessions, reset replay, stale-generation rejection, and `fidelity_gap_flags=0x00` |
| Lifecycle/privacy | Zero bulk managed allocations; seven negative launches; no embedded owner payload |
| Full-frame presentation | Five 1600x900 checkpoints; two byte-identical runs; zero magenta |
| Presentation correspondence | 3 historical original indexed captures, 5 historical managed Unity captures, 23 typed layers, 8 gallery records, plus a separate exact executable native FINAL MAIN timeline; no false cross-timeline pixel/clock or active-DAC RGB claims |
| Reference corpus | Idle 3, active-forward 8, supporting 24, Unity regression 5 |

The ordinary-input determinism run completed at tick 14,720 with:

- command digest `f651ed14282a0377`;
- 94,764 events, digest `85863abfeb25633c`;
- state-sequence digest `b4be8a3d743430ad`.

The same built-player route records all 301 active Speed ticks, including the
240 launch impulse, 160 sustain, and up to nine simultaneous class-10/model-2
source trails. Its first 16 active ticks overlap the already-running Stage 4
objective fade; the remaining active ticks use palette submode zero. Speed
itself never submits a palette request.

The latest focused post-completion ordinary-input route completed alive at
tick 21,039 after observing and fighting group 33, the first multipart creature,
and group 34 before entering the exit. Two fresh sessions produced identical
command, event, state, source-arena, multipart, projectile/impact, mana, audio,
and terminal-state digests. The driver never observes enemy health or actions,
and waits for the row-308 transition through visible world state only.

The focused natural Firefly-retaliation observer uses only ordinary commands
and visible sanitized actor positions. It cannot inspect enemy actions,
health, targets, RNG, projectiles, or pending damage to steer. Its repeated
run observed one hostile spawn, one completed impact and visible impact
object, one source charge contact, and player life falling to 9,410
with identical command, event, and state digests.

Current local artifact identities:

- player launcher SHA-256:
  `8027f7d1f9ae7dacfc826fb218adcc1ae7464af098a8a59397e7531c0f7ec0bc`;
- native plug-in SHA-256:
  `b315562e2b91d4bdef2067a2c26d13ffe82d7f7e7975707f043724984c1f80fa`;
- managed runtime SHA-256:
  `c05d076eb2922bd929790f7fea699fd0d48993d73bc8696fbfd3ff9f451fa577`;
- `globalgamemanagers` SHA-256:
  `8abee05c17b175ff5f5b089ac2fd131f61041b1f320e47891678e07c829ae1d6`;
- Unity build GUID: `7ad6d1a242bf421cb5ddf3bac3c2f6fe`;
- owner-cache content SHA-256:
  `b664ad00c94e3f9393bc92ecac91a821673222cd365ed47617c4fab63db733bf`;
- owner-cache manifest SHA-256:
  `1050b4f1e3f0e63fa292fe7798d19f4b390c063aa5afd5a350fba9e300298082`;
- owner-cache source-frame census SHA-256:
  `8a9fb362badbeb37d0b5ef4dae15be4fcdee92c690e0f2017689c5cf0af31f5e`.

Current evidence from the final rebuilt player:

- PLAN29 acceptance: 1/1 required clean run (run A); run B is not permitted;
- run receipt: `/home/oliver/plan29-final-qualification-20260801/run-a/receipts/run-receipt.json`;
- aggregate receipt: `/home/oliver/plan29-final-qualification-20260801/aggregate.json`;
- final closure receipt: `/home/oliver/plan29-final-qualification-20260801/closure-receipts.json`;
- final closure log: `/home/oliver/plan29-final-qualification-20260801/final-closure.log`;

- self-test: `/tmp/magiccarpet-native-selftest-v41.log`;
- built-player route: `/tmp/magiccarpet-level1-autoplay-v41.log`;
- built-player post-completion route:
  `/tmp/magiccarpet-level1-autoplay-postgame-v41.log`;
- EditMode: `/tmp/magiccarpet-unity-editmode-v30.xml`;
- PlayMode: `/tmp/magiccarpet-unity-playmode-v41.xml`;
- graphical runs: `/tmp/magiccarpet-fullframe-v41-a.fRvg7j` and
  `/tmp/magiccarpet-fullframe-v41-b.36xW3a`;
- objective HUD capture: `/tmp/magiccarpet-objective-visual/hud.png`;
- presentation materialization:
  `/tmp/magiccarpet-presentation-spell-final-v8`.

## Simulation gap status and remaining work

The native mission snapshot reports `fidelity_gap_flags = 0x00`. This is a
legacy bounded mission-simulation register, not a blanket fidelity
percentage. Every bit registered in that older mask is cleared:

- `0x01`: unified source-slot arena;
- `0x02`: exact reachable Castle placement;
- `0x04`: exact Castle construction timing and terrain transaction;
- `0x08`: exact mana-sphere lifecycle;
- `0x10`: exact mana-carrier behavior;
- `0x20`: reachable mission sound emitters, listener-relative commands, pitch,
  play types, and voice arbitration;
- `0x40`: exact exit action, full effective Level 1 palette/fade sidecar, and
  completion sequence;
- `0x80`: secondary groups 33/34 and their reachable creature chain.

The separate resident register currently reports `0x0f` after each frozen
family reaches its unported body:

- `0x01`: house-spawned Archer actions 33–37;
- `0x02`: Builder actions 97–103 and construction action 96;
- `0x04`: Villager actions 105 and 108–110;
- `0x08`: Trader actions 113–119.

Constructors, source-slot ownership, initial action-53 mailbox dispatch, source
projection, collision participation, supported rendering, hashing, cleanup and
reset are implemented. The subsequent action bodies must be ported before this
record may claim complete Level 1 simulation parity.

The owner cache exposes 30 materialized effect records, the explicit silent
sound-12 contract, and the Level 1 music stream. The complete ordinary-input
route reaches Stage 5 without an unknown or unsupported audio command.

Input and presentation fidelity remain separately scoped after the simulation
mask reaches zero. PRS-001 is now exact for the fixed 320×200 Night,
screen-size-0, single-local-player native FINAL MAIN supported COMPLETE states: source tuple and map
chain, hotspot/clipping, reflection/shadow order, dynamic lighting,
temporal/spatial postprocess, overlays, atomic COMPLETE/provenance, C ABI, and
one-plane Unity ownership. Active tutorial/briefing, recursive help popup,
immobilization spider-web, and menu-state-7 score-writer states are reachable
but fail closed before COMPLETE. Remaining presentation nonclaims are
multiplayer-only reveal and hostile/remote players, stereo and alternate
resolution, unsupported source branches and animation modes, active-DAC RGB
for the historical captures, and any cross-timeline pixel or clock equality.
