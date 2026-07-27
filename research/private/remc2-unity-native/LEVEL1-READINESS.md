# Level 1 playable-readiness record

Date: 2026-07-27
Scope: private owner-data preservation build, remc2 source to Rust simulation
to Unity desktop. No DOSBox or original-binary analysis is part of this
milestone.

## Verdict

Level 1 is a complete playable mission slice. The ordinary-input desktop
route starts from a fresh session, flies through all six source-mapped
objectives, collects and casts Fireball, Possession, Castle, and Speed, fights
Archers and Fireflies, builds castle stage 1, possesses and deposits mana
spheres, fills the castle, enters the authored exit, and reaches result code
10 while alive. The separately tested post-completion group-33/34 multipart
chain is implemented in the simulation and presentation, but the current
ordinary-input completion route does not claim to fight that optional chain.

That is a playable-readiness claim, not a frame-perfect-completion claim.
The registered Level 1 simulation fidelity mask is now zero. Every reachable
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

Controls: `W/S` speed, `A/D` strafe, mouse or `I/J/K/L` steering, `C` or
middle mouse centre, `Space` or left mouse fire, `Esc` release capture, click
recapture, `R` restart, `F11` fullscreen, and `F10` quit.

## Working systems

- Exact-integer player flight, terrain queries, spell activation order,
  camera history, death/input suppression, departure, fade, completion, and
  deterministic native state/event contracts.
- Rust-owned objective switches, pickups, Fireball, Possession, Speed,
  castle placement/construction/teardown, mana spheres and carrier, enemy AI,
  damage/death, group-33/34 activation, the authored exit, and result 10.
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
- The six objective textboxes use exact remc2 layout and source draw order:
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
| Rust workspace | 378 passed: atlas 4, C ABI 49, sim core 321, WASM proof 4 |
| Owner cache | 17/17 passed; 447 outputs from 19 source files; deterministic rebuild and corruption negatives |
| Unity EditMode | 26/26 passed, including ordinary-input complete and post-completion combat routes, natural hostile Firefly damage, atomic native palette identity, native frozen-actor fallback gating, and live indexed objective composition |
| Unity PlayMode | 1/1 passed |
| Built-player route | Stage 5, result 10, alive with life 10000 at tick 21,586 |
| Built-player post-completion route | Group 33, first multipart, and all three group-34 creatures fought through ordinary inputs; Stage 5, result 10, alive with life 10000 at tick 41,509 |
| Native self-test | Five checkpoints, two sessions, reset replay, stale-generation rejection, and `fidelity_gap_flags=0x00` |
| Lifecycle/privacy | Zero bulk managed allocations; seven negative launches; no embedded owner payload |
| Full-frame presentation | Five 1600x900 checkpoints; two byte-identical runs; zero magenta |
| Presentation correspondence | 3 registered original indexed captures, 5 registered Unity captures, 23 typed layers, 8 gallery records, no missing references, placeholders, or false cross-timeline pixel/clock claims |
| Reference corpus | Idle 3, active-forward 8, supporting 24, Unity regression 5 |

The ordinary-input determinism run completed at tick 21,586 with:

- command digest `2049cc8884402efa`;
- 146,637 events, digest `65e04fe1a55d2cf6`;
- state-sequence digest `6b2bc40c9842d8ba`.

The optional post-completion ordinary-input route completed alive at tick
41,509 after observing and fighting group 33, the first multipart creature,
and group 34 before entering the exit. Two fresh sessions produced identical
command, event, state, source-arena, multipart, projectile/impact, mana, audio,
and terminal-state digests. The driver never observes enemy health or actions,
and waits for the row-308 transition through visible world state only.

The focused natural Firefly-retaliation observer uses only ordinary commands
and visible sanitized actor positions. It cannot inspect enemy actions,
health, targets, RNG, projectiles, or pending damage to steer. Its repeated
run observed one hostile spawn and impact, nine contacts, and player life
falling to 9,215 with identical command, event, and state digests.

Current local artifact identities:

- player launcher SHA-256:
  `8027f7d1f9ae7dacfc826fb218adcc1ae7464af098a8a59397e7531c0f7ec0bc`;
- native plug-in SHA-256:
  `be69fcff460fd8cb0f05f633a2decc0ddbe97a314ff2325b5f7bc7e137368ccf`;
- managed runtime SHA-256:
  `97d230ceac0b3ec1b72482eb8c79242bbd2e62dbda0cd9f23cd74a1a0d834621`;
- `globalgamemanagers` SHA-256:
  `5057e7f305a81c99040222c0ec501a7b23f3a97552e84f1662efac1ec4dfb8e5`;
- Unity build GUID: `5e6775fe01d848dc8b1d6d4dd22b5d0a`;
- owner-cache content SHA-256:
  `addc1cc1b43173f41f208082237f38b9519fdc64374ec0b20f8b2d9b04049cdb`;
- owner-cache manifest SHA-256:
  `6e8557e06c6a1b9f1cc39c80564c58af24ff804ce1b75859ba2d0ea363c5e09a`.

Current evidence from the final rebuilt player:

- self-test: `/tmp/magiccarpet-selftest-objective.log`;
- built-player route: `/tmp/magiccarpet-level1-built-player-objective.log`;
- EditMode: `/tmp/magiccarpet-editmode-objective-results.xml`;
- PlayMode: `/tmp/magiccarpet-playmode-objective-results.xml`;
- graphical runs: `/tmp/magiccarpet-objective-visual/autoplay` and
  `/tmp/magiccarpet-objective-visual-b`;
- objective HUD capture: `/tmp/magiccarpet-objective-visual/hud.png`;
- presentation materialization:
  `/tmp/magiccarpet-presentation-objective`.

## Simulation gap status and remaining presentation work

The native mission snapshot reports `fidelity_gap_flags = 0x00`. Every
registered Level 1 simulation bit is cleared:

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

The owner cache exposes 30 materialized effect records, the explicit silent
sound-12 contract, and the Level 1 music stream. The complete ordinary-input
route reaches Stage 5 without an unknown or unsupported audio command.

Presentation fidelity remains a separate, visible register after the
simulation mask reaches zero: multiplayer-only minimap reveal, exact sprite
hotspots and remaining animation clocks, and complete original whole-frame
depth/interleaving. The native hit, Speed, objective, exit, fade-back, and
black-fade palette states, the single-player source-ordered minimap pass,
high-resolution HSPRN HUD, live indexed objective textbox, and the shared 4:3
output rectangle are no longer in that deferred register.
