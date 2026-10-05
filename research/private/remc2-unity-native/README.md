# Native Unity Level 1 playable preservation slice

Owner-local Track P presentation harness for Unity `6000.3.20f1`. This is the
playable Level 1 client over the package-backed `sim_capi` ABI v2. The final
readiness verdict, qualification matrix, artifact pins, and explicit
fidelity-gap register are in [`LEVEL1-READINESS.md`](LEVEL1-READINESS.md).
It is not a claim that the complete MC2 engine or original renderer is
finished.

## What is exact

- The caller-supplied, validated package M reset projection and closure binding.
- World `1/0`, Night/outdoor profile, counts `1000/177/8`, package and
  closure hashes, reset state hash `9da4bc91af58f3ce`, preserved package-reset
  conformance hash `77b96b82492d6f4b`, and additive playable-session effects
  live v2 conformance hash `36bd7f1ddf0537b5`.
- All six 65,536-cell terrain planes.
- The 257×257 ground mesh heights (`ground byte * 32` source units), wrapped
  row/column zero duplication, and alternating source diagonals.
- With the qualified owner cache supplied, the source-locked 152-tile Night
  `BL32N0-0` atlas is repacked using remc2's 256-byte stride/address grid and
  sampled by direct terrain-type index through the exact ordered `PALN-0`
  palette. The exact Level 1 selector set contains 65 values with maximum 139.
  Per-cell D4 orientation follows `(terrain angle >> 4) & 7`, the 32-row
  source `UVTable_D4350`, and eight camera-independent Unity mappings.
- Static terrain brightness indexes the first 65,536 bytes of `TABLESN` as
  `TABLESN[(brightness << 8) | source palette index]` before PALN lookup.
  `CLRN-0` is also cache-locked. PALN's RGB6 channels use VGA scaling
  `value * 4` (maximum 252), not bit replication.
- The normal owner-cache view now presents an exact isolated 320×200 indexed
  `sky + terrain` layer from Rust. It seeds the framebuffer with source
  `DrawSky_40950`, builds and projects the 40×21 outdoor lattice, resolves the
  complete reflection pass before primary terrain, executes source mode 5 and
  framebuffer-dependent water mode 26 in submission order, and returns both
  the final index plane and terrain-write coverage. The ABI binds those 64,000
  byte planes to generation and render-camera tick and validates their FNV-1a
  hashes before Unity upload. Unity reverses source top-down rows exactly once,
  uploads persistent point-sampled R8 textures with no mipmaps, and resolves
  every index through the exact ordered PALN palette before opaque diagnostic
  objects. Coverage remains diagnostic because uncovered pixels are the exact
  sky seed required by mode 26.
- All 172 Level 1 scene/live-mission `TMAPS1-0` selectors are cache-locked:
  161 exact RNC decoded payloads and 11 verified one-byte placeholders. The
  four frame-observed selectors are `40`, `41`, `54`, and `63`; complete
  decoded animation streams are preserved alongside first-frame indexed
  images.
- The package-derived presentation catalog assigns all 177 active reset rows
  to 17 exact class/model/action/parameter/particle groups. It proves 130
  terrain-chain heads: 83 pass the reset-state renderer gate, 47 are suppressed,
  and 47 further active rows are off-chain.
- Normal owner-cache mode registers exactly 64 stable managed actor fallbacks:
  33 `GoatLive` goats, 28 `VillagerLive` villagers, and three reset-observed
  dolmen sprites. In the qualified view all 64 are suppressed by native
  compositor capabilities.
  Rust draws their exact TMAPs inside the indexed world frame using source
  map-cell order, facing/mirror selection, projection anchors, clipping,
  reflection-before-primary order, and terrain-projected shadows. Goat
  transforms and lifecycle come from the generation/tick/hash-bound 33-row
  native snapshot; the three `FrozenObserved` dolmens retain their qualified
  reset poses while the 12 selector-global villager clocks advance from actual
  native draw admission.
- Atomic flight snapshots, exact integer simulation, live player row, ordered
  event journal, and eight immutable objective records.
- The source spell-control route is explicit end to end. Menu assignment
  opcodes 31/32 independently bind left and right hands with tier clamping.
  Possession's moving class-10/model-12 marker writes the target building's
  mailbox; the building consumes it in source-slot order and installs the
  persistent player-coloured HSPRN/TMAP marker while advancing Possession
  experience through thresholds 0/600/4800. Terrain or lifetime expiry still
  emits model 12 but cannot possess a building or grant experience. Speed Up
  follows the 240 activation, 160 sustain, 80 restore countdown sequence,
  emits sound 19 only on activation, and creates the no-draw
  class-10/model-2 trail every fourth phase. It does not request a palette
  tint; the green palette belongs to the separate slowing/impairment path.
- With the owner cache supplied, the normal qualified view uses a uniformly
  scaled 640×480 owner HUD reference: Night MSPRN panels, live life/mana
  bars, live Fireball/Possess/Castle/Speed icons 123–126, the source 2×13
  Control-held hand-assignment menu, source-mapped
  five objective selectors 48–52 plus completion selector 53 using HFONT3
  and HSPRN 171/172/173, task
  guidance, player-death/restart, departure, fade, and completion state.
  Each objective box uses its exact source layout and 33-turn clock, blends
  the live indexed world through TABLESN selector 61, then overwrites text
  and frame indices before final palette resolution. The qualified 320×200
  world is pixel-centre-nearest sampled into the 640×480 corrected 4:3
  layout. Unity composes these from native mission snapshots; it does not own
  the progression.
- Its minimap follows the qualified high-resolution HSPRN path: native
  128×128, exact TABLESN scanline aperture, 256 source units per pixel, and
  exact fixed-point remc2 rotation. Rust emits castle-rope pixels first, then
  exact source-slot-ordered entity commands with the Level 1
  class/model/action/control/ownership filters, colors, phase rules, sprite
  anchors, direct/blended writes, objective marks, and one 41-pixel center
  cross. Unity only rasterizes that ordered command stream through the owner
  palette. Multiplayer remote-player reveal remains outside this single-player
  Level 1 claim.
- With the qualified cache, the native world appends exactly twenty-nine
  descriptor-backed audio references (asset-ref count 105 rather than the
  frame-only 76): 28 effect references, including the explicit silent sound
  12 contract, plus the Level 1 music stream. Typed `PrepareSound`, `EndLoop`,
  and `UpdateMusic` events
  carry those indices. Unity loads the exact owner Night bank 1 / quality 822
  WAV records for IDs 1, 2, 3, 4, 5, 8, 10, 15, 16, 18, 19, 27, 29, 31, 33,
  34, 37, 39, 41, 43, 44, 46, 54, 55, 56, 57, and 63; ID 12 is an explicit
  silent mapping, and the final reference is the Level 1 music track.
  Local ambient loops 1/2/5/31, player-local feedback 29, and positional IDs
  37/39/43 all use the original shared ten-effect-voice pool.
  Positional commands arrive in a separate 88-byte sidecar; a missing or
  invalid sidecar is silent.
  Sidecar action `0` means prepared but not yet passed through remc2's
  `PlayEntitySounds` pool and must not be rendered. In the qualified
  single-player/no-CD profile, pause retains those rows; the first unpaused
  outer frame coalesces them with current rows. Ordinary audible winners carry
  a non-zero u64 flush ID in `reserved[0..1]` (the minimum participating event
  sequence) and a contiguous one-based
  `pooledFlushRank`; Unity consumes winners in that order. An event that ages
  out of the 4,096-row journal keeps its sidecar across arbitrary step backlog;
  Unity explicitly releases every resolved row after consuming it. Reset
  invalidates and clears all sidecars.
  Ambient loops start at volume zero, move one integer unit per 60 Hz callback
  toward targets 70/70/120/85, do not restart on repeated prepare events, and
  fade to zero on `EndLoop`. Their original loop count is -1: at zero they
  remain silent, playing, and resident in the shared pool so reactivation
  continues sample phase. Only reset frees them.
- Level 1 logical music track 1 preserves remc2's playback-slot-0 /
  loop-pointer-slot-1 indexing wart. Owner XMIDI is converted through remc2's
  converter, then rendered deterministically to streaming 44.1 kHz stereo
  PCM16 with pinned libADLMIDI bank 59. Playback begins only after the additive
  native `StartMusic` event corresponding to the original second
  `DrawAndEvents` pass at local `gameTurn=1`; reset stops, seeks, clears the
  controller state, and rearms that same gate. This is an owner-local
  presentation adaptation, not exact DOS sound-card timbre.

## Exact fixed-profile frame and diagnostic exclusions

The indexed layer is the accepted FINAL MAIN for supported COMPLETE states in
the registered fixed Level 1 profile. It contains sky and terrain plus every
reachable typed source occupant or an explicit source-derived exclusion, and
it consumes the supported HUD/menu/map/objective/debug writer tree before
COMPLETE. Active tutorial/briefing, recursive help popup, immobilization
spider-web, and menu-state-7 score-writer states are reachable but fail closed
before COMPLETE because their independent live source inputs are not yet typed. The
execution receipt makes this bounded claim machine-readable. It does not
silently widen that claim to the debug carpet, unsupported sprite/overlay
branches, stereo or alternate resolution, hostile/remote players, historical
capture RGB, or cross-timeline pixels and clocks.

The source mode-5 software-raster contract now has a renderer-independent Rust
CPU reference under `sim-core::mode5_raster`. It preserves the original
vertex-order decision tree, quantized 16.16 edge tests, distinct ordinary and
flat-edge divisors, scanline ownership, clipping, wrapped attribute carries,
and per-pixel texture/TABLESN addressing. The reference is qualified with
synthetic split, flat, clipped, byte-wrap, and both-diagonal seam vectors.

Primary Level 1 water uses the separate mode-26 kernel now preserved under
`sim-core::mode26_raster`. It reuses the exact mode-5 triangle setup and
clipping, shades every source texel through TABLESN, and for raw texture
indices 0 through 11 performs the second framebuffer-dependent lookup at
`0x4000 + (existing_index << 8) + shaded_index`. The reference mutates the
indexed framebuffer in source order and is pinned at the direct/blended
threshold, maximum blend address, clipped first sample, repeated submission,
complete triangle, and qualified owner TABLESN vectors.

In explicit debug mode, the former Unity terrain shader remains available as
a depth-only aid for diagnostic object markers. Qualified native presentation
instead owns the accepted fixed-profile 320×200 Night FINAL MAIN, including
source linked-map traversal, reflection-before-primary and shadow-primary
adjacency, hotspot/clipping, dynamic lighting, exact temporal or spatial
TABLESN postprocess, and the supported source overlay writer tree. Unity
uploads that result as exactly one row-flipped point-filtered R8 plane and
suppresses managed presentation only while the current COMPLETE receipt owns
the frame. The historical 1600×900 managed captures and controlled gallery
remain presentation adaptations, not retroactive pixel or clock matches. The
primitive carpet and spell-event sphere/trail remain debug-only. Stereo,
alternate resolution, active-DAC RGB for historical captures, hostile/remote
players, and unsupported source branches remain explicit nonclaims.

Only explicit `-mc-debug` may run without the owner cache. With
`-mc-debug-entities`, numerical/default-font diagnostics and entity labels
return as a test view. They are absent from `-mc-qualified`. The primitive
carpet, lighting, depth proxy, and spell-event visual are debug-only and are
not claims about original MC2 art, projectile simulation, or final asset
fidelity. All objective rows are `FrozenObserved`. Of the 177 active reset
rows, exactly 114 are `FrozenObserved`, 33 are `GoatLive`, 28 are
`VillagerLive`, entity 152 is `PlayerFlightLive`, and entity 153 is
`SpellActivationLive`; the latter's
changing holder fields are not exposed by ABI v2. All 64 qualified reset
actor records are drawn by the native indexed compositor: 33 live Goats, 28
live Villagers, and three reset-observed dolmens. Live Archers are also
native-composited from the mission registry.

Unity owns no gameplay physics. The viewer creates no `Rigidbody`, gameplay
`Collider`, Unity `Terrain`, or PhysX simulation authority.

The additive live mission registry carries 19 authored Level 1 actors,
including four class-3/model-3 multipart heads, six Fireflies, and the
group-34 scroll, plus player Fireballs, Fireball impacts, and Archer arrows
from Rust. Each multipart head owns a source-slot-linked chain of 16 segments.
Archer normal and
attack poses use remc2 particle rows 0, 1, and 206. Normal selectors 8–10
advance on their source-style selector-global clocks; attack selectors
11–13 and 121–123 remain static. Goats and Archers share the same 1,000-slot
map chain as terrain submission, including cross-family same-cell order.
Arrows use particle 195, TMAP facings 105–109, render mode 19, and logical
height 60; their visual remains a Unity presentation adaptation. The 96-byte
mission snapshot exposes a bounded eight-bit mission-simulation fidelity-gap
register; it does not cover input, HUD, renderer, or other presentation
fidelity. The current qualified build
reports `0x00`: the unified-source-pool, exact Castle-placement, exact
Level 1 Castle-construction-timing, and exact Level 1 mana-sphere lifecycle
bits are cleared. The mana-carrier bit is also cleared after source-slot
identity, scheduler, motion, collection/deposit, damage/death, overflow, and
direct-absorption qualification. The exit bit is cleared after exact
source-slot triggering, action-12 movement and timers, stored-history FOV,
integer RGB6 entry palette, framebuffer-wide black fade, input suppression,
result 10, reset, and ordinary-input qualification. The secondary groups
33/34 bit is cleared after qualification of their authored allocation order,
activation switches, four multipart heads and linked segment chains, six
Fireflies, scroll, combat/death/mana paths, exact sounds 8/63, rendering, and
reset. The mission-sound bit is cleared after every reachable event on the
complete ordinary-input route acquired its exact emitter, listener-relative
command, pitch family, play type, and shared-voice arbitration. The registered
Level 1 simulation gap mask is therefore zero.

The unchanged 72-byte event ABI is accompanied by an additive 88-byte effect
sidecar carrying wrapped relative coordinates, raw volume/pan, playback rate,
arbitration action, and shared native voice index. Unity renders the original
integer left/right gains. `spatialBlend`, `panStereo`, Doppler, and rolloff
remain zero/disabled so Unity does not apply a second spatializer. The
`UpdateMusic(mode, 3)` event is consumed and validated; `3` is the source
timer multiplier, producing a 90 Hz integer dynamic-music envelope, not a
group ID. Unity preserves the exact integer 0..127 state, one-unit callback
steps, endpoint retirement, reversal, and reset guard. Playback slot 0 has no
controller-119 tags while AIL pointer slot 1 tags channels 7 and 8. Until an
active reference trace resolves that pointer distinction, Unity leaves the
complete stereo track unchanged and excludes the optional danger OGG rather
than fabricating a combat stem. A playable effect without its complete
matching sidecar is silent and emits
`MC_AUDIO_UNSUPPORTED ... action=suppressed reason=missing_audio_command`.
Genuinely
unknown mappings use the distinct `MC_AUDIO_UNKNOWN` record and also fail
normal qualification. The local-audio harness retains its negative probes;
`-mc-audio-qualify-positional` adds formula, listener, and live shared-pool
qualification.

The reset chain also contains 19 structurally drawable transient rows: 12
Fireball impacts, one explosion/death effect, and six smoke particles. They are
fully mapped to reset selectors 39, 54, and 63, but remain hidden in qualified
mode because original update passes mutate them before the first presented
frame. Debug mode may show their exact frozen capture-site state. The qualified
first-presented-frame trace instead observes `TMAP40×10`, `41×14`, `54×8`,
and `63×23`; live transient animation/link/lifetime parity is not claimed.

Goat and villager TMAPs retain their complete four-frame delta streams. Rust
owns the three Goat and twelve villager selector-global clocks, mutable
indexed pixels, preceding-draw latches, wrap, reset, and unpaused service
order. Native CPU draw admission supplies the feedback; unseen selectors
freeze. Typed sound-46 requests use the qualified source numeric pitch:
slot 102 observes stored `0x9909f858`, peeks nonstored `0xa631b437`, and emits
action-14 `+25` or other-action `+5` without changing authoritative RNG. No
presentation behavior is guessed.

Owner packages are never copied into `Assets`, `StreamingAssets`, or the built
player. A player must receive paths explicitly:

```text
-mc-qualified
-mc-package /absolute/path/to/level1-export-m
-mc-closure /absolute/path/to/level1-asset-closure.json
-mc-asset-cache /absolute/path/to/level1-unity-asset-cache-plan29-final
```

`MC_PACKAGE_PATH` and `MC_CLOSURE_PATH` are equivalent environment fallbacks.
Only the Unity editor defaults to the private sibling `generated/` paths.

The private decoded owner-asset cache is required by `-mc-qualified`. It is
optional only for the explicit `-mc-debug` fallback. Pass
`-mc-asset-cache PATH` or set
`MC_ASSET_CACHE_PATH`. Unity validates its schema, exact package/closure
bindings, every size and SHA-256, and its aggregate content identity before
accepting it. The cache remains external; raw DAT/TAB files are never loaded.
The current qualified identity is 1,249 outputs, manifest SHA-256
`1050b4f1e3f0e63fa292fe7798d19f4b390c063aa5afd5a350fba9e300298082`
and content SHA-256
`b664ad00c94e3f9393bc92ecac91a821673222cd365ed47617c4fab63db733bf`.
Its `frame/source-frame-census.json` SHA-256 is
`8a9fb362badbeb37d0b5ef4dae15be4fcdee92c690e0f2017689c5cf0af31f5e`
and is part of the exact FINAL MAIN admission contract.

TMAP actor selection and native indexed presentation are qualified and loaded.
The cache preserves all TMAP animation groups and complete delta streams and
now materializes the exact four mutable Goat states for selectors 152–154
against the pinned remc2 oracle. Rust owns the exact independent
selector-global latches, frame indices, cursors, mutable pixels, wrap, reset,
and unpaused service order for the qualified Goat, villager, Archer, Firefly,
combat, and castle clocks. Families without a qualified native clock remain
visibly static or absent.
Records 426–436 are one-byte `b'0'` placeholders, not decoded textures and not
the TAB record-504 offset sentinel.

## Rebuild and install the native plugin

```bash
cd /home/oliver/Projects/odcpw/magiccarpet/research/private/remc2-sim-rs
cargo build -p sim-capi --release --locked
install -Dm755 target/release/libsim_capi.so \
  /home/oliver/Projects/odcpw/magiccarpet/research/private/remc2-unity-native/Assets/Plugins/x86_64/libsim_capi.so
```

## Build the Linux player

```bash
/home/oliver/Unity/Hub/Editor/6000.3.20f1/Editor/Unity \
  -batchmode -nographics -quit \
  -projectPath /home/oliver/Projects/odcpw/magiccarpet/research/private/remc2-unity-native \
  -executeMethod MagicCarpet.Native.Editor.BuildNativePlayer.BuildLinuxPlayer \
  -logFile /tmp/magiccarpet-unity-build-v2.log
```

## Run the v2 headless qualification

```bash
Builds/Linux/MagicCarpetNative.x86_64 \
  -batchmode -nographics -mc-qualified -mc-self-test \
  -mc-package /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-export-m \
  -mc-closure /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-asset-closure.json \
  -mc-asset-cache /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-unity-asset-cache-plan29-final \
  -logFile /tmp/magiccarpet-native-selftest-v2.log
```

The test locks managed layouts, both hashes, counts, all planes, objectives,
two independent sessions, reset-generation invalidation, the deterministic
eight-forward route, live player row agreement, event ordering/fire/cursor,
modular wrap arithmetic, exact-once session destruction, the pre-entity-update
camera row, additive look/FOV inputs, reset camera history, and the additive
72-byte `DrawWorld_411A0` terrain-slope camera sidecar. The renderer-independent
Rust tests separately pin its four-height sampling, half-tile/wrap selection,
±100 target clamps, signed `>>3` persistent easing, reset, terrain divisor
floor/rejection boundaries, and strict object depth `>64`. The Unity
`nearClipPlane=0.035` is explicitly logged as a raster-safety adaptation; it is
not presented as an equivalent of any of those original projection rules.
The test also locks the complete HUD selector/dimension/authority contract.
With the cache supplied it also locks
the 109-ref native bridge, known-only typed audio vector, descriptor parsing,
streaming music extent, zero/ramp/repeated-prepare/fade loop envelope, the
managed 40/96-byte terrain-frame ABI layouts, exact reset index/coverage
hashes, same-tick equality, independent-session equality, row-flip
involution, and reset/replay stability.

## Run the viewer

Campaign menu / loading-screen UI:

```bash
sh ./run-campaign.sh
```

To enter any of the three real owner levels directly:

```bash
sh ./run-campaign.sh --level 1
sh ./run-campaign.sh --level 2
sh ./run-campaign.sh --level 3
```

Levels 2 and 3 load Rust-generated entry replays with one neutral opening turn.
Level 3 carries Fireball, Possession, Castle and Speed for independent play,
including the Possession needed to claim mana spheres.
The Rust loader verifies the exact package and state receipts. These entries
let you play each level independently;
they do not establish completion of the preceding campaign objectives. The
usual New Game button still starts the campaign at Level 1.

Campaign controls: `W/S` accelerate/brake, `A/D` strafe, mouse or `I/J/K/L`
steer, and `C` centers steering. `1` selects Fireball, `2` Possession, `3`
Castle and `4` Speed; `Space` casts the selected spell. Rust rejects spells
you have not acquired. Mouse buttons cast the retained left/right hand spells.
`Escape/P` pauses, `F5` saves, `R` restarts and `F10` quits.

This uses the original validated indexed loading artwork with functional Unity
buttons. Escape/P opens the paused menu; Resume, Save, Restart Level, Main Menu
and Quit dispatch to the existing Rust runtime. New Game visibly passes through
Loading; Continue remains subject to Rust's real exit-completion check, not just
the objective counter. Button artwork/layout is not yet an original-menu port.
`MC_CAMPAIGN_SAVE` selects a save file (default: ignored `generated/campaign-save.json`).
Each run prints its separate log directory. `MC_CAMPAIGN_REPLAY` optionally starts
a saved game directly rather than the main menu.

The campaign now draws Level 1's qualified Night terrain, sky, live sprites and
original status/spell HUD through the same Rust session used by the Day levels.
Pause, reset and Title→New Game retain that session's authority and loading gate.
Night minimap entity/objective overlays use native commands and Night symbols
and colors; the current objective instruction comes from the original owner
text selected by Rust. The integrated Level 1→2→3 playthrough remains unfinished.
The legacy viewer below remains a separate preservation entry point.

The owner-local launcher supplies the qualified package, closure, and external
asset-cache paths:

```bash
./run-level1.sh
```

Set `MC_FULLSCREEN=1` for fullscreen, or override `MC_WIDTH` and `MC_HEIGHT`.
Additional Unity/player arguments are passed through.

The shared `run-player.sh` launcher selects Unity's native Wayland backend when
`WAYLAND_DISPLAY` is set, avoiding the observed X11 window-event startup stall
under KWin Wayland. It leaves X11 sessions unchanged. Set
`MC_WINDOW_BACKEND=default` to retain Unity's own selection, or `wayland` to
request Wayland explicitly. Other campaign invocations can use
`sh ./run-player.sh <player arguments>` without changing their package inputs.

The corrected Day campaign uses the spell-complete v3 caches below. Run from
this Unity project directory. This replay enters Level 2 directly for play/debug;
it is not evidence of completing the preceding campaign objectives.

```bash
sh ./run-player.sh -mc-game-runtime \
  -mc-runtime-package ../generated/mc-cf15-level1-package-v3-current \
  -mc-runtime-package ../generated/level2-day-block32-campaign-v2 \
  -mc-runtime-owner-cache ../generated/level2-day-spell-owner-cache-v3/components/material \
  -mc-runtime-presentation-cache ../generated/level2-day-spell-owner-cache-v3 \
  -mc-runtime-package ../generated/level3-day-block32-campaign-recipe-v2 \
  -mc-runtime-owner-cache ../generated/level3-day-spell-owner-cache-v3/components/material \
  -mc-runtime-presentation-cache ../generated/level3-day-spell-owner-cache-v3 \
  -mc-runtime-replay-load ../generated/opened-l2-block32-replay-v5.json \
  -screen-fullscreen 0 -screen-width 1600 -screen-height 900
```

Historical minimap-only caches remain readable, but lack Fireball texture 463;
use the spell-complete caches with the current projectile presentation code.

The equivalent explicit invocation is:

```bash
Builds/Linux/MagicCarpetNative.x86_64 \
  -mc-qualified \
  -screen-fullscreen 0 -screen-width 1600 -screen-height 900 \
  -mc-package /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-export-m \
  -mc-closure /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-asset-closure.json \
  -mc-asset-cache /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-unity-asset-cache-plan29-final
```

Use `-mc-debug -mc-debug-entities` instead of `-mc-qualified` to expose and
label all 177 active registry rows,
including frozen reset-effect sprites and unresolved diagnostic control
proxies, and to restore the diagnostic numerical `OnGUI`. `-mc-qualified`
shows the owner HUD plus only the 64 evidence-backed dolmen, goat, and villager
sprites. Its player-origin registry marker remains hidden, and no substitute
carpet is rendered. Debug mode retains the temporary foreground carpet aid.

Controls: held `W/S` state is sampled once per authoritative 30 Hz simulation
turn; every sampled turn changes the speed target by one source step. `A/D`
strafes, the mouse steers persistently, `I/J/K/L` provides keyboard steering,
and `C` or middle mouse centres. Holding `Control` opens remc2's 2×13 bottom
spell menu and releases pointer capture: left click chooses the left hand and
right click chooses the right hand. Keep that button held, move horizontally
across the three-tier submenu, and release to send opcode 31/32; locked
families and tiers cannot be assigned. Releasing `Control` restores capture.
Outside the menu, `Space` or left mouse casts the left-hand spell and right
mouse casts the right-hand spell.
`Esc` releases capture, click recaptures, `R` resets, `F11` toggles fullscreen,
and `F10` quits.

## Full Level 1 input-only playback

The Level 1 acceptance driver uses the ordinary player command path and a
sanitized, player-visible observation contract. It cannot read enemy health,
actions, targets, RNG, cooldowns, pending damage, or AI state. The optional
fast flag pumps multiple ordinary fixed ticks between rendered frames; it does
not change simulation ordering or command generation.

```bash
xvfb-run -a -s "-screen 0 1600x900x24" \
  Builds/Linux/MagicCarpetNative.x86_64 \
  -mc-qualified \
  -screen-fullscreen 0 -screen-width 640 -screen-height 480 \
  -mc-level1-autoplay -mc-level1-autoplay-fast \
  -mc-level1-autoplay-quit \
  -mc-package /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-export-m \
  -mc-closure /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-asset-closure.json \
  -mc-asset-cache /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-unity-asset-cache \
  -logFile /tmp/magiccarpet-level1-built-player.log
```

The current accepted route completes at tick 14,720, enters Stage 5, and
receives exit result 10. Omitting `-mc-level1-autoplay-fast` runs the identical
controller at normal presentation speed. The fresh-session conformance test
currently records command digest `f651ed14282a0377`, 94,764 events with
digest `85863abfeb25633c`, and state-sequence digest `b4be8a3d743430ad`.
The route also records the complete 301-tick Speed holder lifecycle: 240
launch speed, 160 sustain, and up to nine simultaneous class-10/model-2
source trails. It suppresses forward/reverse input while Speed owns the target
because remc2 treats either as an explicit cancellation request.

Add `-mc-level1-autoplay-postgame` to delay entering the exit and exercise the
optional source-authored post-completion encounter. The controller activates
group 4 through the row-305 world trigger, fights the visible group-33
Firefly, the first multipart creature, and all three visible group-34
multipart creatures, then exits. It still receives only player-visible
observations and issues ordinary player commands; it cannot inspect the
row-308 dying-chain state, enemy health, AI actions, targets, RNG, cooldowns,
or pending damage. The accepted two-session route completes at tick
21,039 and separately binds command, event, state, source-arena, multipart,
projectile/impact, mana, audio, and terminal-state digests. Negative controls
also prove that skipping the row-305 overlap, leaving the first multipart
creature alive, or leaving any group-34 creature alive cannot complete the
optional route.

## Graphical autoplay and captures

```bash
MC_AUTOPLAY_CAPTURE_DIR=/tmp/magiccarpet-autoplay-v2 \
  Builds/Linux/MagicCarpetNative.x86_64 \
  -mc-qualified \
  -screen-fullscreen 0 -screen-width 1600 -screen-height 900 \
  -mc-package /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-export-m \
  -mc-closure /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-asset-closure.json \
  -mc-asset-cache /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-unity-asset-cache \
  -mc-autoplay -mc-autoplay-quit \
  -logFile /tmp/magiccarpet-autoplay-v2.log
```

For the required run A, verify every manifest and recompute every PNG hash and
magenta count:

```bash
python3 verify_qualified_full_frames.py /tmp/run-a
```

The verifier retains an optional second-directory comparison for historical
developer determinism work. PLAN29 final acceptance is exactly one clean run
(run A); run B is neither required nor permitted by the final qualification
policy.

Autoplay asserts turning, altitude change, typed fire events, 177 registry
rows, zero runtime `Rigidbody`/`Collider`, the absence of every diagnostic
presentation object in qualified mode, and writes five 1600×900 captures.
Qualified mode deliberately has no approximate carpet proxy, so its logged
clearance diagnostic is `Infinity`.
Each checkpoint is armed inside the authoritative fixed simulation step. The
simulation is then frozen until `WaitForEndOfFrame` captures the displayed
frame, so checkpoints 145, 163, 220, 303, and 400 cannot silently drift to a
later tick. Every PNG has a JSON manifest binding its checkpoint and native
render tick, command-prefix identity, post-step state/conformance row, typed
event range and logical hashes, pre-update presentation/render-camera rows,
terrain-frame identity, package/closure/cache/plugin identities, Unity build
and graphics identity, PNG and raw-RGBA SHA-256, dimensions, and zero-magenta
result. The durable reference registry and executable verifier live in
`qualification/`; they explicitly separate pinned remc2 evidence from the
synthetic native/Unity route.
The post-fire captures at ticks 163 and 303 retain the input-latch and typed
kind-4 event evidence. Qualified mode renders Rust-owned Fireball, arrow,
castle-projectile, Possession, and impact snapshots inside the native indexed
frame. It never substitutes the explicit debug sphere/trail marker; that
marker remains available only in `-mc-debug`.
Water is represented by the exact terrain-type plane coloring; there is no
coplanar presentation-only water sheet.

The current Level 1 integration run is recorded in:

- `/tmp/magiccarpet-unity-build-exact-captures-v2.log`: Unity Linux player
  build PASS, 86,136,066 bytes;
- `/tmp/magiccarpet-native-selftest-exact-captures.log`: headless
  `MC_SELF_TEST PASS`;
- `/tmp/magiccarpet-autoplay-exact-capture-b.log` and
  `/tmp/magiccarpet-autoplay-exact-capture-c.log`: two graphical
  `MC_QUALIFIED_AUTOPLAY PASS` at tick 440 with all five captures, 64 qualified
  owner sprites, no Rigidbody/Collider, exact terrain-frame route checks, one
  source-turn music start, and `presentationClean`, `physicsClean`,
  `terrainFrameClean`, `carpetOverlayClean`, `firePresentationClean`, and
  `diagnosticObjectsClean` all true, plus `magentaClean=5/5`.
- `/tmp/magiccarpet-qualified-full-frame-verification.log`:
  `MC_QUALIFIED_FULL_FRAMES PASS checkpoints=5 exact=5 magenta=0
  byte-identical-runs=2`.
- `verify_owner_hud.py` on tick 145 versus tick 400: PASS with zero magenta
  pixels, zero bottom-right near-white overlay pixels, all exact owner bitmap
  samples present, and 43,273 changed minimap pixels.

The later Level 1 reference-corpus qualification is recorded in:

- `/tmp/magiccarpet-unity-build-level1-qualification-final-v4.log`: current
  qualified Linux player build PASS;
- `/tmp/magiccarpet-native-selftest-level1-qualification-final-v4.log`:
  `MC_ROUTE_QUAL PASS` for five checkpoints, 440 steps, two independent
  sessions, reset replay, stale-generation rejection, state, camera, terrain,
  and typed event identities;
- `/tmp/magiccarpet-level1-reference-corpus-verification-final-v4.log`:
  original idle frames 0/8/19, all eight active forward passes, all 24
  supporting recorded transitions, and all five synthetic Unity checkpoints
  PASS with explicit evidence classifications;
- `/tmp/magiccarpet-qualified-full-frame-level1-final-v4.log`: two final
  schema-v2 graphical runs are byte-identical at all five exact checkpoints
  with zero magenta pixels. The full-frame gate also binds entity/profile,
  terrain generation/layout/viewport, exact Unity version and build GUID.
- `/tmp/magiccarpet-presentation-v4.tmkwl0/`: historical capture review
  evidence with three base-PALN original-frame decodes, five independently
  clocked managed Unity captures, a typed 23-layer correspondence contract,
  and a separate fifth executable native FINAL MAIN timeline; the historical
  managed/gallery layers remain adapted, and
  `MC_LEVEL1_PRESENTATION PASS` with zero cross-timeline pixel/clock claims,
  placeholders, missing owner references, or magenta pixels.

Neutral native replay reproduces the original flight projection exactly at
idle frames 0, 8, and 19. The live hashes at frames 8 and 19 are
`061b7ffc82131a7e`/`a6b40ecb8b1d373c`; after normalizing only the additive
native session tick, the source-projection hashes are the original
`0e4547b42a98c976`/`13f7f355fcf6738f`. Both contracts remain visible so a
lifecycle clock is never mistaken for a 1995 state field.

Both typed fire requests remain observable and feed the Rust-owned Level 1
projectile simulation and owner-TMAP presentation. The diagnostic
sphere/trail remains available only with explicit `-mc-debug`;
`-mc-debug-entities` additionally shows unresolved entity diagnostics.

The runtime-supplied owner-data boundary is now an executable gate:
`qualification/verify_unity_lifecycle_privacy.py`. It requires the exact
31/31 EditMode composite and 1/1 graphics-backed PlayMode results; pins the
launcher, runtime
assembly, global managers, plug-in, and build GUID; checks Unity build
metadata; rejects StreamingAssets, symlinks, owner container/audio extensions,
and registered owner-payload hash intersections; and reruns the full
built-player self-test. Seven negative launches cover missing inputs and
package/closure/cache corruption. Both batch and normal graphical
missing-input starts exit 2, so a failed player never remains as a frozen,
apparently runnable window.

A straight-ahead autoplay baseline approached a genuine steep terrain face
around tick 80. The retained presentation route now turns from its first tick;
the exact simulation and camera are not changed or cheated. Whether remc2
applies a missing forward-probe/collision response on the straight route
remains an explicit later qualification question.
Semantic prefab replacement, resolved tagged-channel dynamic music, exact
remaining source sprite anchors and animation clocks, multiplayer minimap
reveal, and wider-map qualification remain later work. The source-mapped
Level 1 HSPRN HUD, indexed objective-textbox blend, minimap entity/mark pass,
transient palette, and live progression are qualified now.

## Owner-audio mixer qualification

The runtime's post-listener capture tap is dormant unless
`MC_AUDIO_CAPTURE_PATH` is explicitly set. The dedicated local-audio
qualification path disables base music, freezes simulation stepping, drives
typed events through the normal presentation code, and advances its stages
from the captured audio sample clock rather than render-frame time:

```bash
MC_AUDIO_CAPTURE_PATH=/tmp/magiccarpet-local-audio-qual.wav \
  xvfb-run -a Builds/Linux/MagicCarpetNative.x86_64 \
  -mc-qualified \
  -screen-fullscreen 0 -screen-width 640 -screen-height 480 \
  -mc-audio-qualify-local \
  -mc-package /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-export-m \
  -mc-closure /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-asset-closure.json \
  -mc-asset-cache /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-unity-asset-cache \
  -logFile /tmp/magiccarpet-local-audio-qual.log
```

The local run emits one `MC_AUDIO_WINDOW` record per bounded listener interval
and cannot report `MC_AUDIO_LOCAL_QUAL PASS` unless all 24 local windows pass.
Raw transition markers remain in the evidence, while silent steady-state
measurement begins after one fixed 20 ms audio block so a callback already in
flight cannot be misclassified as post-transition playback. Amplitude and RMS
thresholds remain unchanged.
It covers:

- initial silence and the increasing ocean start-at-zero ramp;
- ocean-to-crickets crossfade continuity and eventual mutual exclusion;
- crickets, Fire, and Market fade-to-silence while their loop voices remain
  resident;
- Fire/Market near-versus-far loop-command outcomes as a separate
  already-resolved 1536-unit accumulator path;
- repeated Market prepare without source restart or a silent 20 ms block;
- one audible CANTUSE player-local feedback event followed by silence;
- explicit NULL, positional-missing-sidecar, and unknown negative windows remaining
  silent with distinct structured diagnostics; and
- an audible generation-tagged loop followed by a real native session reset,
  generation increment, and silent cleanup window.

The separate positional and shared-voice qualifier uses the same paths with
`-mc-audio-qualify-positional`. It emits 17 positional/live-allocator windows
and proves exact
left/right gains, near/far energy, measured low/high-rate signal behavior,
natural completion/reuse, ten first-free allocations, eleventh-effect
no-steal, duplicate suppression, and reset while the pool is full:

```bash
MC_AUDIO_CAPTURE_PATH=/tmp/magiccarpet-positional-postfix.wav \
  xvfb-run -a Builds/Linux/MagicCarpetNative.x86_64 \
  -mc-qualified \
  -screen-fullscreen 0 -screen-width 640 -screen-height 480 \
  -mc-audio-qualify-positional \
  -mc-package /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-export-m \
  -mc-closure /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-asset-closure.json \
  -mc-asset-cache /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-unity-asset-cache \
  -logFile /tmp/magiccarpet-positional-postfix.log
```

One observed 24/24 run produced PCM16 stereo at 48 kHz for 18.026667 seconds:
1,730,560 interleaved samples, 1,271,430 nonzero samples, peak 0.668132, and
temporary capture SHA-256
`8f1725b319fb3d18ebbaed05fa3ad26c283ab699665f3d9be4d14bbffb8de026`.
An independent second run again passed all 24 windows while producing
17.792000 seconds, 1,708,032 samples, 1,242,834 nonzero samples, and a
different observed hash
`b63493b7053287a15b7f15fbe0413d830291db44729a49b578c8e448bc7781ee`.
The window verdicts and command/state invariants are the qualification
contract; the whole-file hash is scheduling-dependent private evidence, not a
golden.

The final signed-loop-count rebuild passed the local qualification 24/24 at
`/tmp/magiccarpet-local-loop-count-final.log`, the positional/shared-pool
qualification 17/17 at
`/tmp/magiccarpet-positional-loop-count-final.log`, and the headless
self-test at `/tmp/magiccarpet-selftest-loop-count-final.log`. The local
run additionally proves that a faded ambient loop stays resident on the same
voice, advances phase while silent, resumes through `UPDATE_EXISTING` without
another play start, and occupies the shared pool until reset.

The final camera-history/slope/raw-yaw build passed the headless gate at
`/tmp/magiccarpet-native-selftest-raw-yaw-final.log`, the five-capture
graphical gate at `/tmp/magiccarpet-autoplay-raw-yaw-final.log`, local audio
24/24 at `/tmp/magiccarpet-local-raw-yaw-final.log`, and
positional/shared-pool audio 17/17 at
`/tmp/magiccarpet-positional-raw-yaw-final.log`.

The exact pnt5-grid build passed 25 native-v2 tests, the Unity player build at
`/tmp/magiccarpet-unity-build-pnt5-grid.log`, the headless gate at
`/tmp/magiccarpet-native-selftest-pnt5-grid.log`, and the five-capture
graphical gate at `/tmp/magiccarpet-autoplay-pnt5-grid.log`. The graphical
run reports 840 unique source vertices, 612 eligible four-corner cells for the
reset view, 13 CPU/GPU pnt5 boundary vectors, and keeps the screen-linear
mode-5 stage explicitly deferred.

The qualified isolated indexed terrain-frame build passes all 30 native-v2
tests and 159 sim-core tests, the Unity build at
`/tmp/magiccarpet-unity-build-terrain-qualified-order-final.log`, the headless
gate at
`/tmp/magiccarpet-native-selftest-terrain-qualified-order-final.log`, and two
independent five-capture graphical gates at
`/tmp/magiccarpet-autoplay-terrain-qualified-order-a.log` and
`/tmp/magiccarpet-autoplay-terrain-qualified-order-b.log`.

The isolated/no-ambient reset frame and managed self-test retain index hash
`2f38beef8e4a62fb`. The complete reset world has native index hash
`6cbc716bdcd04ed6`, while both paths retain coverage hash
`65fa674ae4afefec`, 578 primary mode-5 triangles, and 37,174 terrain writes.
GPU qualification pins the full-world raw R8 upload hash
`a572934d2db1f794`, PALN-resolved bottom-up readback hash
`ad0386b8aca8afbd`, and logical top-down RGBA hash `77fdd7ddf04bb56d`.
It reads back all 64,000 raw index pixels, all 64,000 raw coverage pixels, all
256 PALN entries, and all 64,000 resolved RGBA pixels, then repeats the reset
readback at the same tick and records the graphics API/device.

The reset-camera ambient ablation keeps coverage bytes, coverage hash, and
terrain statistics byte-identical. The complete ambient slice changes exactly
5,362 index pixels within `(25,114)-(252,199)`. Isolated model families change
1,482 pixels for model 1 within `(57,138)-(99,178)`, 102 pixels for model 14
within `(134,114)-(211,168)`, and 4,309 pixels for model 0 within
`(25,134)-(252,199)`. Their respective framebuffer-write counts are 1,482,
104, and 4,810; the complete slice writes 6,396 pixels because source-ordered
families may overwrite prior pixels. Emitter 113 is off the renderable map
chain and its isolated ablation contributes zero framebuffer writes and zero
changed pixels.

Rust qualification also pins a reset-lattice census of 672 visible / 168
culled vertices, 388 near / 212 fog-ramp / 72 fog-zero visible vertices, both
checkerboard parities at 336 each, and all eight D4 groups in every distance
band. Consecutive low-water turns pin moving-wave index/coverage hashes with
36 reflected and 300 mode-26 triangles. Explicit `render_y=65535` and
`render_y=0` vectors pin the toroidal seam.

The deterministic live route pins native index, coverage, triangle/write, and
GPU RGBA hashes at ticks 145, 163, 220, 303, and 400. Tick 220 first exercises
mode 26; tick 400 exercises 924 reflected, 214 primary mode-5, and 36 primary
mode-26 triangles. Each checkpoint writes a bounded private 320×200 RGBA PNG,
source-order R8 index plane, source-order R8 coverage plane, and JSON manifest
under the requested system-temporary capture directory. The v2 manifest names
the source-top-down R8 order, Unity bottom-up GPU readback storage order, and
logical top-down PNG order separately and pins both RGBA hashes. The final
local evidence is under
`/tmp/magiccarpet-terrain-qualified-order-a.OW4TJF/`; it is private ephemeral
owner-data evidence, not a distributable artifact. All 20 bounded terrain
artifacts were byte-identical to the independent capture under
`/tmp/magiccarpet-terrain-qualified-order-b.3sgBOd/`.

The earlier base-music/autoplay capture path remains useful as a whole-view
smoke test:

```bash
MC_AUDIO_CAPTURE_PATH=/tmp/magiccarpet-unity-listener-mix.wav \
  xvfb-run -a Builds/Linux/MagicCarpetNative.x86_64 \
  -mc-qualified \
  -screen-fullscreen 0 -screen-width 640 -screen-height 480 \
  -mc-autoplay -mc-autoplay-quit \
  -mc-package /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-export-m \
  -mc-closure /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-asset-closure.json \
  -mc-asset-cache /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-unity-asset-cache \
  -logFile /tmp/magiccarpet-audio-listener-player.log

ffprobe -v error \
  -show_entries stream=codec_name,sample_rate,channels,duration \
  -of json /tmp/magiccarpet-unity-listener-mix.wav
ffmpeg -hide_banner -nostats \
  -i /tmp/magiccarpet-unity-listener-mix.wav \
  -af volumedetect -f null -
```

One observed qualified run produced 9.600 seconds of PCM16 stereo at 48 kHz,
868,327 nonzero interleaved samples, peak 0.157493, mean -30.9 dB and maximum
-16.1 dB. The capture SHA-256 was
`cb64476cce28c7a99b19b916e8b0ca406303bb1bba0cfd6a525b681349dc7299`.
A second reviewed run had the same format, duration, peak and volume metrics,
but 870,375 nonzero samples and SHA-256
`f76584994984bf0999f5dc21ae6651b88a844fbc2045995f9372dad6b337642f`.
Unity mixer scheduling makes these observed capture hashes non-repeatable
evidence, not golden artifacts. The deterministic pre-mixer music WAVE hash
`03b3f951…4605d` remains the repeatable golden.
The final suppression/confinement build was captured again: 9.557333 seconds,
917,504 samples, 864,231 nonzero, the same 0.157493 peak / -30.9 dB mean /
-16.1 dB maximum, and observed SHA-256
`f68c59fffd38549728948c8dbb039d412333fe781fe09e0938fa9d6f36fbb61b`.
It is owner-derived private evidence: keep it out of `Assets`, `Builds`, and
all distributable repositories, then delete it after review.

The capture writer accepts only direct descendants of the system temporary
directory whose existing parent chain contains no symlink/reparse point. It
rejects Unity `Assets`, `StreamingAssets`, `Builds`, parent escapes,
nonexistent parents, symlinked parents, and directory targets before opening
the output. Capture-path preflight runs synchronously in
`MagicCarpetRuntime.Awake`; an invalid requested path produces `MC_FATAL`,
nonzero exit, no autoplay PASS, and no output file.
`-mc-audio-qualify-local` additionally requires both an armed capture path and
the exact owner cache. A missing or failed capture request exits 2 before any
local-audio PASS marker.

## Controlled owner-HUD capture

The normal owner view contains no Unity/default-font HUD. The Linux player is
built without Unity's `Development Build` overlay. A deterministic capture and
moving-pose comparison can be produced with:

```bash
capture_dir=/tmp/magiccarpet-owner-hud
mkdir -p "$capture_dir/autoplay"
MC_HUD_CAPTURE_PATH="$capture_dir/hud.png" \
MC_HUD_COMPARISON_CAPTURE_PATH="$capture_dir/hud-moving.png" \
MC_AUTOPLAY_CAPTURE_DIR="$capture_dir/autoplay" \
  xvfb-run -a Builds/Linux/MagicCarpetNative.x86_64 \
  -mc-qualified \
  -screen-fullscreen 0 -screen-width 1600 -screen-height 900 \
  -mc-package /absolute/path/to/level1-export-m \
  -mc-closure /absolute/path/to/level1-asset-closure.json \
  -mc-asset-cache /absolute/path/to/level1-unity-asset-cache \
  -mc-autoplay -mc-autoplay-quit

python3 verify_owner_hud.py "$capture_dir/hud.png" \
  --comparison "$capture_dir/hud-moving.png"
```

The verifier checks the 640×480 uniform reference transform; exact palette
samples from the minimap/health/blank/player/icon panels and both spell frames
plus Fireball/Possess icons; the exact Night red HFONT3 objective, live
TABLESN-blended indexed underlay, HSPRN frame, and frozen badge; owner minimap
diversity; centered live player marker; stable foreground objective/top-bar
pixels across moving captures; a changed
player-centered/yaw-rotated minimap; no magenta shader failure; and no
bottom-right Unity/default-font overlay. The specific autoplay capture tick is
reported by the runtime and can change if the presentation capture cadence is
adjusted.

## Controlled entity gallery

The test-only gallery does not step or mutate the simulation. It places the
exact dolmen, three goat orientation samples (including a rear-sector UV
mirror), and all four villager appearances at controlled distance and common
baselines:

```bash
MC_ENTITY_GALLERY_CAPTURE_PATH=/tmp/magiccarpet-entity-gallery.png \
  Builds/Linux/MagicCarpetNative.x86_64 \
  -mc-debug \
  -screen-fullscreen 0 -screen-width 1600 -screen-height 900 \
  -mc-package /absolute/path/to/level1-export-m \
  -mc-closure /absolute/path/to/level1-asset-closure.json \
  -mc-asset-cache /absolute/path/to/level1-unity-asset-cache \
  -mc-entity-gallery -mc-autoplay-quit

python3 verify_entity_gallery.py /tmp/magiccarpet-entity-gallery.png
```

The verifier locks each projected quad against its TMAP dimensions and
`rotSpeed / 64` scale, requires visible bounds and PALN color diversity,
compares the side goat against the rear UV mirror, and rejects magenta shader
failures.
