# Successor stack decision

Version: 0.3  
Status: exact planning pin adopted; Unity is the Track S baseline and the
browser-native stack is a measured fallback  
Track: `S`  
Decision owner: Oliver

## Decision

Begin Track S with Unity 6 LTS, C#, URP, Unity Web, a measured Linux headless
authority, Blender LTS plus deterministic `bpy`, and a small TypeScript browser
shell. Keep the canonical simulation in Unity-free C# assemblies whose state
uses integers, fixed point or explicitly specified quantization and does not
depend on `UnityEngine`, PhysX result order, rendering or wall-clock time.

This is a bounded, reversible baseline—not permission to build the whole game
before the proof gate. It is selected because the project needs excellent
graphics, authoritative multiplayer, Blender assets, efficient agent/editor
iteration, later Windows/Linux native builds and later OpenXR VR. Unity's
integrated workflow is directly valuable to those needs.

The MC2 preservation stack is not automatically the successor stack.

## Current planning pin

As of 2026-07-23:

- The exact planning pin is Unity `6000.3.20f1`, changeset `c9ba695d4f07`;
  Unity lists the 6.3 LTS family as supported through December 2027.
- Unity CLI is useful but still experimental. Pin its exact beta, retain
  ordinary Editor batch-mode build scripts, and treat the CLI as replaceable
  automation rather than a project dependency.
- Editor control uses reviewed, separately pinned official Pipeline and Coplay
  MCP paths. All important operations must also have reproducible source,
  batch-mode build or test commands. Neither editor-control path enters a
  player or server build.
- Unity Personal is currently free for eligible game developers below Unity's
  USD 200,000 revenue/funding threshold. Recheck eligibility and terms at each
  commercial gate.
- Unity Web's managed C# runtime is single-threaded, direct IP sockets and
  `System.Net` are unavailable, Web Audio is limited, and cross-platform
  physics results are not guaranteed identical. The baseline therefore uses a
  browser-compatible secure transport and does not make PhysX canonical.
- OpenXR remains a later native presentation/input provider through Unity's XR
  plug-in framework; XR packages do not enter the first browser proof.

The exact package, server, protocol, automation and proof contract is
[`successor-stack-pin.md`](successor-stack-pin.md).

## Baseline U — Unity

Provisional shape:

```text
Unity 6 LTS + URP
  Client presentation, input, audio, UI, editor, tools
  Integer/fixed/quantized simulation assemblies without UnityEngine/PhysX authority
  Unity Dedicated Server versus .NET 10 authoritative-host bakeoff
  Unity Web client over secure browser transport
  OpenXR adapter later
```

Strengths:

- mature editor and profiler;
- official CLI plus batch/execute-method automation;
- active MCP bridges for agent-controlled scenes, scripts, assets and tests;
- strong Blender/FBX/GLB, animation, lighting, terrain, Shader Graph, VFX,
  audio and UI workflows;
- one scene/content pipeline across web/native/VR;
- large talent, asset and documentation ecosystem;
- OpenXR and platform tooling.

Risks to prove:

- Web bootstrap and memory;
- browser GC and single-thread/AOT constraints;
- Web audio, background-tab and filesystem lifecycle;
- server build size and separation;
- deterministic simulation accidentally depending on PhysX or Unity timing;
- package/licensing churn;
- MCP/CLI reliability and reproducibility;
- large-city streaming and destruction on minimum Chrome hardware.

## Fallback W — browser-native renderer plus native/Wasm core

Provisional shape:

```text
Rust or C++ deterministic core
  -> native Linux authority
  -> Wasm browser client
TypeScript + Three.js/Babylon/WebGPU presentation
Blender -> glTF/GLB validated asset pipeline
```

Strengths:

- direct browser control and potentially smaller bootstrap;
- native/Wasm core parity;
- worker/thread and memory ownership can be explicit;
- web networking/storage/service-worker ecosystem is direct;
- easier DOM/UI and web deployment;
- no editor/runtime license dependency.

Risks:

- more engine/editor/tooling must be built;
- weaker integrated animation, VFX, lighting, terrain and VR authoring;
- agent automation edits code/data but lacks a comparable whole-scene editor;
- native/VR requires another presentation shell or packaged web technology;
- asset and shader workflow complexity moves onto this project.

## Contingency G — Godot

Strengths:

- open source;
- strong native scene/editor workflow;
- GDExtension precedent through MagicBalls;
- capable 3D and OpenXR path;
- Blender/glTF-friendly pipeline.

Risks:

- MagicBalls does not prove web readiness;
- web threading, C++ extension, memory and networking require direct proof;
- current project would need to qualify agent automation, browser build,
  server separation and large-world performance;
- smaller ecosystem than Unity for some production needs.

## Core-language decision

Do not decide “Rust versus C# versus C++” globally. Decide by responsibility.

Potential division:

| Responsibility | Leading option | Why |
|---|---|---|
| Unity client/editor | C# | Native Unity workflow, tooling, serialization |
| Unity successor simulation | Unity-free integer/fixed/quantized C# initially | One build path and easiest Web compatibility if determinism and performance pass |
| Browser-native challenger core | Rust | Strong native/Wasm portability, explicit data ownership, server reuse |
| Preservation core | Existing C++ | Fidelity and existing reconstructed corpus |
| Web shell/tools | TypeScript | Browser APIs, deployment, room UI, diagnostics |
| Asset generation/validation | Python + Blender `bpy` | Deterministic headless authoring and broad tooling |
| Performance kernels | Evidence-driven | No native plug-in in Unity Web baseline; use only after measured need |

Rust is neither rejected nor forced. It becomes a runtime candidate only after
a reproduced integer/fixed determinism or performance failure survives two
focused remediation days, or if the browser-native challenger wins.

## Baseline proof scene

Unity must run this original synthetic proof first. A fallback candidate is
built only after Unity misses a declared hard gate and must then run the same
workload:

- one 4 km bounded heightfield;
- carpet movement, bank, skim, climb/dive and projectile;
- 100 ordinary actors plus burst to the candidate demo budget;
- deterministic resource ledger and objective;
- terrain query/edit and dirty regions;
- streamed original Blender static/rigged assets;
- water, vegetation, volcano, clouds/storm, day/dusk, spatial audio;
- HUD/minimap;
- four Chrome clients against Linux authority;
- reconnect, snapshot, replay and first-divergence logs.

No candidate may substitute a simpler workload for its weakest area.

## Measured decision axes

| Axis | Required evidence |
|---|---|
| Agent iteration | clean setup, CLI/MCP use, scene edit, compile, play, test, error recovery |
| Browser bootstrap | compressed bytes, requests, cold/warm time to control |
| Memory | Wasm/managed heap, browser process, GPU/assets, growth over match/rematch |
| Frame performance | low/quality presets on the declared minimum and T14 |
| Determinism | native/server/browser command replay and state hashes |
| Server | no-GPU Linux build, idle/match memory and CPU, multiple rooms |
| Networking | secure browser transport, prediction, snapshots, repair, reconnect |
| Assets | Blender source to validated runtime artifact and provenance manifest |
| Visual quality | terrain, water, weather, lighting, VFX, animation, UI |
| City path | streamed tile/block proof and bounded destruction |
| VR path | OpenXR/WebXR feasibility, camera/input separation, comfort |
| Operations | reproducible build, CI, hosting, logs, failure artifact, rollback |
| Licensing | engine/tool/package/source obligations and commercial suitability |

## Decision rule

Retain Unity when it meets browser/server budgets and gives meaningfully
faster, more reliable production through its editor, CLI/MCP, Blender
integration, graphics, content tooling and OpenXR path.

Invoke the browser-native fallback only when Unity misses hard browser/server
budgets after bounded remediation, or its automation/content advantages do not
compensate for a measured failure.

A failed CLI, MCP, transport or server adapter does not count as a Unity client
failure. Replace the failed adapter first. The exact thresholds and remediation
ladder are in [`successor-stack-pin.md`](successor-stack-pin.md).

Evaluate Godot only if both primary paths fail for a named reason. MagicBalls
is Track P integration evidence, not a reason to choose Godot for Track S.

The decision records a primary stack and a fallback. It does not forbid
specialized offline tools or a future second presentation client.
