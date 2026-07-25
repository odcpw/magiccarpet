# Future vision: native VR carpet flight

Status: later Track S native-platform exploration; not first social-demo scope  
Date: 2026-07-23  
Depends on: accepted native deterministic parity and a proven flat-screen
carpet experience

## Vision

Run the same successor simulation and original game content as a native VR
experience: seated or standing on a magic carpet, looking freely across the
world, aiming spells with an approved controller scheme, reading a spatially
reworked HUD and fighting human or AI opponents without changing authoritative
game rules.

The natural first target is native PC VR through OpenXR. Browser VR is not a
requirement. Standalone headsets are a later hardware, thermal, memory and
package decision rather than an assumed downscale of the PC build.

Unity's XR Plug-in Management and OpenXR provider architecture make this a
credible integration path:

- [Unity XR Plug-in Management](https://docs.unity3d.com/6000.0/Documentation/Manual/xr-plugin-management.html)
- [Unity XR architecture](https://docs.unity3d.com/6000.0/Documentation/Manual/XRPluginArchitecture.html)

## Architectural rule

VR is a presentation, tracking and input adapter over the same:

- `Simulation.Core`;
- successor ruleset and world provider;
- fixed-tick authority;
- save/replay formats;
- networking protocol;
- flight, spell, AI and objective laws.

Head pose, eye pose, controller pose, comfort vignette, stereo cameras and
local haptics are not canonical world truth. A VR client emits the same bounded
gameplay commands as a flat-screen client.

Optional replicated head/hand/avatar poses are presentation channels. Losing
them may reduce embodiment but must not change hits, movement, visibility,
mana, objectives or victory.

## Input and embodiment

The XR adapter should support evidence-based alternatives:

- seated carpet posture with a stable carpet/cockpit reference;
- controller-stick flight using the flat-screen action map;
- head-look independent of carpet heading;
- controller, gaze-assisted or reticle-based spell aiming;
- optional physical gesture flavor that resolves to ordinary validated cast
  commands;
- recenter, handedness, seated/standing calibration and reachable UI;
- pause and headset-loss behavior that is safe in solo and multiplayer.

Do not make neck direction secretly steer the carpet by default. Looking,
aiming and steering must remain independently tuneable so combat does not
require uncomfortable head motion.

## Comfort is a product gate

Magic Carpet's speed, altitude change, strafing, turning, impacts and horizon
motion make it a high-risk comfort experience. “It runs in a headset” is not an
acceptable completion condition.

Required comfort options include candidates for:

- stable carpet nose, frame or cockpit reference;
- horizon stabilization;
- reduced roll and camera shake;
- acceleration and angular-velocity comfort profiles;
- snap and smooth turning where applicable;
- dynamic peripheral vignette/tunneling;
- comfort-sensitive boost, knockback and falling presentation;
- seated mode, recentering and physical boundary awareness;
- reduced flashes, particles, weather and camera impulses;
- instant return to a known stable view after respawn or reconnect.

Comfort changes may alter local presentation and input transfer functions, but
competitive profiles must define which movement capabilities remain equivalent
and how accessibility is protected.

## HUD, map and spell selection

The flat-screen HUD cannot simply be pinned to both eyes.

VR investigation should compare:

- carpet-mounted or wrist-like spell selection;
- world-locked versus head-locked status elements;
- diegetic compass, minimap and objective cues;
- readable depth, scale, contrast and focal distance;
- damage direction and off-screen threat indication;
- subtitle, notification and multiplayer identity placement;
- non-gesture fallback for every required action.

All required information must remain available without forcing repeated extreme
neck movement or precise mid-air pointing.

## Rendering and performance

The exact headset matrix and minimum refresh rate are selected by a later
hardware gate. Each admitted device must sustain its frame timing under
representative flight, combat, weather, terrain edits and destruction.

The production investigation includes:

- OpenXR runtime and device validation;
- single-pass instanced/multiview stereo where supported;
- XR-safe shaders, post-processing, particles and screen-space effects;
- late latching/tracking behavior provided by the selected XR stack;
- resolution scale and optional fixed or gaze-driven foveated rendering;
- CPU, GPU, memory, thermal and dropped-frame evidence;
- flat-screen/native parity and clean XR-package disablement.

Unity documents single-pass stereo as a way to reduce stereo CPU cost and
supports foveated rendering on selected XR platforms. Both remain measured
optimizations rather than required game truth:

- [Unity stereo rendering](https://docs.unity3d.com/6000.0/Documentation/Manual/SinglePassStereoRendering.html)
- [Unity foveated rendering](https://docs.unity3d.com/6000.0/Documentation/Manual/xr-foveated-rendering.html)

## Multiplayer

The authoritative server remains headset-agnostic. A room compatibility vector
may require an admitted VR client build, but cannot fork simulation rules merely
because a participant uses a headset.

Tests must cover:

- VR and flat-screen clients in the same room;
- input-rate, prediction and correction behavior;
- head/controller pose loss;
- pause/headset removal and reconnect;
- avatar pose abuse and impossible tracking values;
- accessibility and competitive-readability implications;
- bandwidth caps for optional embodiment channels.

## Relationship to voxel worlds and cities

VR and volumetric destruction are independent future branches. Either may ship
without the other.

A destructible city in VR compounds the hardest costs: stereo rendering,
high-speed optic flow, structural collapse, particles, dust, streaming and
comfort. Their integration therefore requires a separate combined gate after
both the VR flight slice and the flat-screen destructible-city block pass on
their own.

## Validation ladder

1. **Empty OpenXR build** — headset starts, stops and returns to flat mode
   cleanly.
2. **Synthetic carpet room** — head-look, steering, recentering, aiming and
   stable frame timing.
3. **Flight comfort slice** — altitude, turns, strafe, acceleration, collision,
   knockback, fall and respawn across comfort profiles.
4. **Combat slice** — Fireball, target acquisition, HUD, audio, haptics,
   weather and accessibility.
5. **Mixed-client multiplayer** — VR and flat-screen clients share one
   authoritative room and replay.
6. **Full representative match** — measured performance, comfort, usability and
   failure recovery on admitted hardware.
7. **Optional combined experiment** — only after the independent destructive
   voxel/city gate passes.

Each rung records exact Unity/XR/OpenXR/runtime/device versions, render mode,
refresh target, build hash, settings, profiles, dropped frames, comfort
incidents and the first failed criterion.

## Explicit non-goals

- Browser VR as a compatibility requirement.
- Rewriting gameplay rules for VR.
- PhysX or headset tracking as authoritative simulation.
- Mandatory gestures without controller/button alternatives.
- Assuming PC performance proves standalone-headset viability.
- Letting speculative XR packages enter the initial browser build.
- Combining VR and city destruction before each passes independently.
