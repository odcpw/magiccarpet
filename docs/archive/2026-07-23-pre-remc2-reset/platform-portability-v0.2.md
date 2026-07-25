# Cross-platform targets, portability rules, and build gates

> Historical decision: superseded on 2026-07-23 by
> `docs/program-charter.md`, `docs/platform-portability.md`, and
> `docs/stack-decision.md`. Its detailed Unity Web evidence remains a useful
> input, but its single-product premise is inactive.

Version: 0.2  
Status: accepted MC2 full-game and modular-engine baseline  
Tracking bead: `mc-7e2`  
Depends on: product promise v0.4  
Decision date: 2026-07-23

## Decision

The first deliverable is a browser game, not a native game wrapped in a browser
afterward. The same authoritative simulation and protocol must nevertheless run
in browser, native client, and headless-server proof lanes from the first
technical slice.

This is also a full-game and module-delivery constraint. The 25-realm MC2
module, a wholly synthetic conformance module, and supported declarative mods
must use the same AOT-safe host and package contracts. MC2 catalog IDs, realm
counts, rules, assets, UI facts, and save fields may not be hard-coded into
engine or platform assemblies. Chrome never downloads managed assemblies or
native plug-ins through the baseline mod path.

The baseline is:

- Unity 6 LTS and URP as the candidate client/server toolchain; current
  official evidence makes the 6.3 LTS family the leading candidate, not a pin;
- current Stable 64-bit Google Chrome, WebGL2, and WebAssembly on Windows and
  Linux as the Tier 1 browser surface;
- Windows 11 x86-64 and Ubuntu 24.04 LTS x86-64 as Tier 1 native-client
  certification surfaces after the browser demo;
- Ubuntu 24.04 LTS x86-64 as the Tier 1 authoritative server surface;
- Unity Dedicated Server as the initial server host; a pure-managed host is a
  later measured replacement decision, not an implicit second baseline;
- authoritative/shared correctness never requires managed threads, native
  plug-ins, Rust FFI, runtime code generation, direct browser sockets,
  GPU-authoritative state, or platform-specific simulation behavior.
  Web-safe shared code runs single-threaded. Native/server-only adapters may
  use bounded concurrency only when accepted results and Web fallbacks remain
  identical.

`mc-gzn` must select the Unity LTS family and pin one exact editor revision,
Hub revision, modules, packages, checksums, licenses, and rollback point.
Nothing in this document authorizes installing Unity or creating a project
before that decision.

## Module portability invariants

- A room and save identify the exact engine API, module set, schema versions,
  dependency graph, content roots, and rules hashes.
- The Linux authority validates and allowlists that complete set before
  admission. Native and Chrome clients run identical authoritative rules.
- Chrome acquisition is content-addressed and transactional, with quota
  preflight, integrity validation, interrupted-download recovery, cache
  eviction handling, rollback, revocation, and safe module removal.
- Module behavior is bounded declarative data/graphs on the browser baseline.
  Reflection-only registration, runtime code generation, downloaded C#,
  native plug-ins, and target-specific rule forks fail the gate.
- Saves, checkpoints, and replays either migrate through an explicit module
  migration or remain safely blocked with exact remediation; they never guess.
- Full-campaign cold/warm loads, total and peak memory, storage, patch size,
  and all realm transitions receive measured budgets before 1.0.

## Target tiers

| Surface | Demo status | Required environment | Acceptance role |
|---|---|---|---|
| Chrome on Windows | Tier 1 | Current supported Windows 11 x86-64 release, current Stable 64-bit Chrome, WebGL2/Wasm, no experimental flags | Player-facing release and minimum-hardware gate |
| Chrome on Linux | Tier 1 | Ubuntu 24.04 LTS x86-64, current Stable 64-bit Chrome, WebGL2/Wasm, X11 or Wayland as recorded | Player-facing release and minimum-hardware gate |
| Native Linux client | Required engineering/conformance lane; external Tier 1 after demo | Ubuntu 24.04 LTS x86-64; Vulkan baseline with supported-driver evidence | Fast debugging and parity replay |
| Native Windows client | Build-smoke target when its host exists; external Tier 1 after demo | Supported Windows 11 x86-64; Direct3D 11 baseline | Portability warning and future package; not demo distribution |
| Linux dedicated server | Tier 1 | Ubuntu 24.04 LTS x86-64, no GPU, Unity Dedicated Server subtarget | Sole authority |
| Steam Deck | Post-demo internal compatibility target | One locked physical Deck model/software matrix; compare native Linux and Windows-under-Proton packages | Controller, 1280 x 800, suspend/resume, performance, packaging proof |
| Edge, Firefox, Chromium variants | Compatibility watch | Current public release, best-effort smoke | Detect needless Chrome coupling; not a demo blocker |
| macOS Chrome/Safari and native macOS | Architected, not certified | Real Apple hardware and CI required before promotion | Later gate |
| Native PC VR through OpenXR | Post-MC2 exploration; never a browser requirement | Exact headset, OpenXR runtime, native OS/GPU and comfort/performance matrix selected later | Same simulation/module over an XR presentation/input adapter; see `future-native-vr.md` |
| Mobile browser/native, standalone headset, console | Non-target | None | Separate hardware and roadmap decision |
| WebGPU | Experimental enhancement only | Feature-detected, never required | May improve presentation after WebGL2 passes |

Oliver's current EndeavourOS workstation is a productive development and smoke
environment, but current Unity 6.3 documentation names Ubuntu 22.04 and 24.04
for Linux Editor/Player support. EndeavourOS evidence cannot close an Ubuntu or
Windows certification gate. No unusual install prefix, wrapper, container, or
compatibility layer is introduced merely to disguise that distinction.

SteamOS 3 is Arch-based and is not covered by Unity's Ubuntu support statement.
The internal Steam Deck gate therefore requires a real device and an explicit
native-Linux versus Proton decision; it is not a claim of Valve
Deck Verified/Playable status. The evidence locks Deck model, SteamOS build,
kernel/Mesa, Steam Client channel, Proton version where used, power/TDP mode,
dock/display state, and package choice. A Chrome-for-Testing `linux64` run is
not Deck evidence.

## Chrome version policy

“Current Chrome” is a moving support policy, not an unrecorded floating binary.

### Player policy

- Support the current Chrome Stable channel on Windows 11 and Ubuntu 24.04 LTS.
  The initial Windows certification image is Windows 11 25H2 x86-64; a later
  supported feature release replaces it through a reviewed platform-lock
  update rather than silently extending support for an expired build.
- Require 64-bit, WebGL2, WebAssembly, hardware acceleration, and default
  production flags.
- Detect capabilities rather than trusting the user agent: create a WebGL2
  context, inspect required extensions/limits, compile representative shaders,
  validate required Wasm features, and report the actual renderer.
- Fail preflight with a useful result for missing features, blocked GPU/driver,
  32-bit browser, disabled WebAssembly, or insufficient memory.
- Do not use Canary, Dev, command-line feature overrides, `chrome://flags`, or
  software rendering to make a release gate pass.

### Reproducible automation policy

- Resolve Stable and Beta from the official Chrome for Testing JSON API in a
  scheduled browser-update change.
- Pin the complete four-part version, revision, platform artifact URL, locally
  computed and recorded SHA-256, and matching driver in the repository
  toolchain lock. Verify that checksum on every subsequent fetch; Chrome for
  Testing does not publish an upstream checksum.
- Pull requests run the pinned Stable binary. Release candidates additionally
  run consumer Chrome fully updated from the vendor Stable distribution
  available to that host on real Windows and Linux hardware. Record its exact
  patch; equality with the Chrome-for-Testing lock is not required during a
  phased Stable rollout.
- A software-rendered SwiftShader lane may provide deterministic headless
  contract smoke, but it is labeled as such and never substitutes for a headed
  real-GPU performance, shader, memory, or release result.
- A scheduled Beta lane is advisory until a failure is reproduced on Stable;
  it supplies warning before the next browser promotion.
- When Stable changes, open a bounded lock-update change, run browser gates,
  retain the old lock in build provenance, and promote the new lock only after
  it passes. Never silently update the CI binary inside an unrelated run.
- Every browser artifact records full browser version, channel, OS/build,
  GPU/driver, WebGL renderer, command line, flags, display server, build ID,
  protocol/schema IDs, cache state, and whether the run was headed.

On 2026-07-23 this workstation reports consumer Google Chrome
`150.0.7871.124`; the official Chrome for Testing dashboard reports Stable
`150.0.7871.129`. These are observations, not permanent minimum versions.

## Minimum and reference hardware

### Minimum browser/native client

The reference floor to acquire or access before playable-demo acceptance is:

- Intel NUC 11 Performance `NUC11PAHi5`;
- Core i5-1135G7;
- Intel Iris Xe 80-EU integrated graphics;
- 8 GiB dual-channel RAM;
- supported Windows 11 and Ubuntu 24.04 LTS installations;
- exact initial Windows and Ubuntu graphics-driver versions pinned in the
  platform lock when the reference machine is acquired, then advanced only
  through a reviewed lock update;
- 1,280 × 720 low preset at a 30 fps cap.

No CPU/GPU throttling or software renderer substitutes for this physical class.
An equivalent device may replace the exact NUC only through a recorded
CPU/GPU/RAM comparison and approval.

### Quality workstation

Oliver's T14 is the 1,920 × 1,080 quality target:

- Intel Core Ultra 7 258V;
- Intel Arc 130V/140V graphics;
- 32 GiB RAM;
- 60 fps ordinary-flight/combat target.

### Server

- one authoritative four-human match plus AI on 2 x86-64 vCPU and 2 GiB RAM;
- no GPU requirement or presentation assemblies;
- Ubuntu 24.04 LTS base;
- the exact CPU model, virtualization/container environment, kernel, image
  digest, and throttling policy are recorded for performance evidence.

## Load, memory, and storage budgets

These are provisional allocation caps, included because platform feasibility
needs an envelope before package and content decisions. `mc-w58.13`, `mc-vgm`,
and `mc-cm9` own measured final budgets and may tighten or replace a number
through an explicit evidence-backed scope change; they may not silently loosen
the product's load or minimum-hardware promise.

| Budget | Technical proof | Playable demo |
|---|---:|---:|
| Compressed first-visit web bootstrap | <= 50 MiB | <= 250 MiB |
| Uncompressed initial `.data` | <= 64 MiB | <= 128 MiB |
| First visit to flight at 25 Mbit/s down | <= 60 s | <= 120 s |
| Cached return to flight | <= 30 s | <= 60 s |
| Unity Web heap, steady in-match | <= 256 MiB | <= 512 MiB |
| Unity Web heap, transient peak | <= 512 MiB | <= 768 MiB |
| Browser tab private memory, steady | <= 768 MiB | <= 1.0 GiB |
| Browser tab private memory, transient peak | <= 1.0 GiB | <= 1.25 GiB |
| Native-client working set, steady | <= 1.0 GiB | <= 2.5 GiB |
| Native-client transient peak | <= 1.5 GiB | <= 3.5 GiB |
| Server resident memory, one match | <= 512 MiB | <= 1.5 GiB |
| Installed/cached demo content | <= 500 MiB | <= 2 GiB |

The playable-demo network reference is 25 Mbit/s down, 5 Mbit/s up, at most
80 ms round-trip time to the server, and at most 1% steady packet loss. Runtime
bandwidth, tick, and terrain-repair budgets belong to `mc-5nc` and
`mc-w58.13`.

The numbers above are allocation envelopes, not a complete measurement
specification. Before P5 can pass, `mc-w58.13` and `mc-cm9` must accept exact
collectors and markers: used versus committed Unity heap; Chrome tab/process
tree attribution; sampling interval and maximum/percentile rule; cache/service
worker state; network shaper and server location; start at navigation request;
interactive at accepted input plus audio/pointer readiness; flight at the first
authoritative controllable-carpet tick; and cold/cached run count. Informal
Task Manager readings cannot close a budget.

A growing WebAssembly heap can fail when the browser cannot allocate a new
contiguous block. A passing average is insufficient: acceptance captures heap,
browser-process memory, assets, graphics allocations, garbage collection,
long frames, and a complete match-plus-rematch. Cache eviction or unavailable
IndexedDB must cause a recoverable re-download, never lost authoritative state.
The initial configured Web heap maximum must not exceed 2,048 MiB; the normal
path is expected to remain far below it.

## Portability invariants

### Authoritative simulation

- `Simulation.Core` is pure C# with no `UnityEngine`, rendering, audio,
  platform SDK, filesystem, wall clock, locale, or input-device dependency.
- The authoritative server owns entity, mana, objective, terrain, RNG, damage,
  ownership, spawn, death, and victory state.
- Canonical time is integer tick/sequence state. A host scheduler may wake the
  simulation, but wall-clock timers never decide gameplay.
- Canonical values declare widths, signedness, units, quantization, overflow,
  rounding, coordinate frames, byte order, ordering, and migration.
- Unity/PhysX results are not authoritative. Unity documents accumulated
  WebAssembly/native physics differences; flight, projectiles, terrain
  collision, and gameplay queries therefore use the independently specified
  deterministic core.
- Rendering, particles, animation, audio, interpolation, camera, input feel,
  and local prediction may differ in presentation without changing accepted
  commands or world hashes.
- One seed, command stream, content hash, schema version, and rules profile must
  produce the same authoritative hashes in managed tests, Chrome, the required
  Linux native conformance client, and Linux server. Windows native joins this
  equality gate before external Windows distribution.

### Web-safe managed baseline

- Treat managed C# as single-threaded on Web.
- Do not use `System.Threading`, thread-backed timers, timeout overloads whose
  implementation requires those timers, blocking waits, synchronous network
  loops, or order that depends on task scheduling.
- Do not use `System.Reflection.Emit`, runtime code generation, runtime assembly
  loading, unsupported reflection-only serialization, or link-strip-sensitive
  type discovery without generated registration.
- Use explicit tick scheduling, bounded work queues, preallocated/pool-backed
  hot paths, and deterministic iteration order.
- Native C/C++ Web multithreading and its cross-origin-isolation headers are
  excluded from the baseline. A later experiment must prove material value and
  hosting/embedding consequences before enabling them.
- Native plug-ins and Rust FFI are excluded from browser and shared simulation.
  A future native/server acceleration path requires a C# or Wasm fallback,
  identical vectors, packaging matrix, measured need, and explicit approval.

### Network

- Browser gameplay uses secure WebSocket or the selected Unity Web-compatible
  transport over HTTPS; browsers never attempt direct TCP/UDP, `System.Net`
  sockets, or ICMP ping.
- Prefer same-origin HTTPS/WSS for the demo. Cross-origin Fetch or content
  requires exact CORS allowlists, never wildcard credentials.
- Native and browser adapters expose the same ordered command/snapshot/repair
  contract even if their underlying transports differ.
- Application messages provide RTT/health data because browser ICMP is
  unavailable.
- Disconnect, duplicate, delay, reorder, cache reload, refresh, hidden tab,
  suspend, and process death are normal lifecycle inputs, not undefined states.

### Storage and lifecycle

- Browser storage and cache are optional accelerators. They are not the sole
  home of authoritative match, ownership, progression, or compatibility state.
- All filesystem access goes through a small asynchronous storage adapter.
  Paths never rely on case folding, writable install directories, drive
  letters, path separators, current working directory, or atomic rename
  semantics that are absent on another surface.
- Content packages are immutable and content-addressed. Mutable user settings
  and resumable state are schema-versioned and bounded.
- Versioned content activation is atomic. A service worker or cache update must
  not combine old code/protocol with new data; an in-match client is told an
  update is ready and reloads only at a safe boundary. The previous complete
  compatible set remains available for rollback until no active client needs
  it.
- Page visibility, focus, audio context, pointer lock, fullscreen, gamepad
  connection, WebGL context loss, and online/offline changes enter through
  platform lifecycle events.
- Treat a hidden browser tab as a long stall. Unity documents that even with
  `runInBackground`, Web updates may fall to roughly one per second, and a host
  may pause them entirely. Tests cover 5-second, 30-second, and 2-minute
  background intervals followed by server snapshot repair.
- The server applies the separately specified safe-carpet/reconnect policy when
  a client stops producing timely commands.

### Input, graphics, and audio

- Gameplay consumes named input actions, never raw device assumptions.
  Keyboard/mouse is the demo baseline; gamepad paths exist from the first
  flight proof.
- Test QWERTY, QWERTZ, and AZERTY bindings. Unity Web does not support all
  physical-to-active keyboard-layout mapping.
- WebGL2/OpenGL ES 3-class shader capability is the presentation floor.
  Compute, geometry/tessellation, platform-only shader features, and GPU
  readback cannot be required for gameplay.
- Every required shader compiles and renders a golden scene on WebGL2,
  Direct3D 11, and Vulkan before entering the shared library.
- Audio starts only after a user gesture. The browser soundscape uses the
  Web Audio-backed capabilities actually proved in Chrome: layered clips,
  priority/voice limits, basic positioning, rolloff, pitch, and Doppler.
- Browser-incompatible mixer DSP or dynamic audio streaming cannot be required
  for the demo. Pre-rendered variants, crossfades, and state-driven layers are
  valid fallbacks.
- Missing optional presentation support degrades deliberately and visibly; it
  never changes authoritative rules.

## Assembly and adapter boundaries

The exact repository layout is chosen in `mc-w37`, but these dependency rules
are mandatory:

| Logical assembly | May depend on | Must not depend on |
|---|---|---|
| `Simulation.Core` | generated contracts, deterministic math/data | Unity, platform, IO, network transport, presentation |
| `Game.Contracts` | primitive schema/runtime support | Unity objects, platform SDKs |
| `Platform.Abstractions` | contracts | concrete browser/native APIs |
| `Client.Unity` | core, contracts, abstractions, presentation | server-only administration |
| `Client.Web` | client and browser adapters | native sockets, native plug-ins |
| `Client.Native` | client and native adapters | browser JavaScript assumptions |
| `Server.Core` | core, contracts, server abstractions | Unity, URP, audio, UI, input, camera |
| `Server.UnityHost` | server core and minimum Unity Dedicated Server APIs | URP, audio, UI, input, camera |
| `Editor.Tools` | schemas and editor APIs | runtime inclusion |

Platform code is selected by assembly/build profile and injected interface, not
scattered conditional compilation through simulation logic. A source rule or
analyzer must reject forbidden references and unsupported API families before
the first gameplay implementation.

## Dependency, package, and shader admission matrix

Every Unity package, managed library, SDK, shader package, native tool, and
runtime plug-in needs one reviewed row before use:

| Field | Required evidence |
|---|---|
| Identity | name, source, exact version/commit, archive/hash, owner |
| Rights | license, notices, redistribution, generated-output terms |
| Role | editor-only, build-only, client runtime, server runtime |
| Platform | Web, Windows, Linux client, Linux server, later surfaces |
| Runtime shape | managed/native, threads/tasks, reflection/codegen, unsafe, Burst/Jobs, subprocess |
| Web/AOT | IL2CPP/linker result, Wasm support, browser APIs, memory behavior |
| Graphics/audio | required API, shader stages, texture/audio formats, fallbacks |
| Network/storage | sockets, Fetch/WebSocket, CORS, filesystem/cache assumptions |
| Build | exact modules, scripting defines, stripping/link metadata, deterministic output |
| Verification | focused tests, empty-build proof, runtime proof, known failures |
| Exit | isolation, replacement, removal, data migration, re-review trigger |

Evidence states are explicit:

1. **Claim-reviewed:** authoritative compatibility claim/source, rights,
   expected platform shape, fallback, and no known blocking red flag. This is
   sufficient only for P1 admission to a throwaway build proof.
2. **Build-proven:** the exact resolved package compiles/links into its named
   P2 profile with AOT/linker diagnostics retained.
3. **Runtime-proven:** focused hosted/target tests pass in the gate that owns
   the dependency's behavior.

An exact runtime dependency without a claim-reviewed Web row cannot enter P2.
It cannot enter the playable demo until its required Web row is runtime-proven.
Editor-only tools must be physically excluded from runtime/server assemblies
and builds. “Works in Editor” is not evidence of Web, AOT, linker, or server
compatibility.

`mc-gzn` also selects and locks the exact required Wasm feature set
(baseline/MVP versus WebAssembly 2023, BigInt, table behavior, and any other
Unity player setting). The same lock drives build configuration, capability
preflight vectors, supported-browser policy, and failure messages.

## Distribution and signing posture

### Demo

- External friend distribution is one private HTTPS browser URL with expiring
  or replaceable room access.
- Web build files use content hashes and an immutable build manifest. HTML and
  version pointers revalidate; versioned Wasm/data/bundles can be immutable.
- Brotli/compression MIME and content-encoding headers, range behavior,
  IndexedDB/cache behavior, WebSocket upgrade, TLS, CSP, CORS, and any
  cross-origin-isolation headers are tested on the real host, not Unity's basic
  local Build-and-Run server.
- A deployment is reversible: retain the previously accepted build and server
  protocol, expose maintenance/incompatible states, and support one-command
  rollback at the hosting layer.
- The browser artifact is not code-signed; HTTPS identity, build hashes,
  provenance manifest, deployment record, and access control are the integrity
  mechanism.

### Native and server during the demo

- Native clients are internal test artifacts only. They carry hashes, build and
  provenance manifests, but no public continuity promise.
- The Linux server is an immutable, digest-addressed artifact or image with a
  software bill of materials and deployment record.
- Dedicated Server optimizations do not excuse inspection: presentation
  assemblies/assets are excluded deliberately, including explicit exclusions
  if Addressables or another Scriptable Build Pipeline enters the graph.
- No unsigned native binary is distributed to friends as a workaround for a
  failed browser build.

### Post-demo external native release

- Windows external packages require an approved Authenticode signing identity
  and timestamping path.
- Linux external packages require signed checksums/release metadata; container
  images require a pinned digest and approved signature/provenance path.
- Steam/Deck distribution, platform accounts, signing secrets, crash upload,
  auto-update, installer/uninstaller, and store obligations require a separate
  release gate. They are not implicitly authorized by building a native player.

## Build profiles and evidence

The project must produce these named profiles from one pinned source revision:

1. `Web-Dev`: diagnostics, assertions, uncompressed symbols where safe.
2. `Web-Release`: IL2CPP/Wasm, WebGL2, release stripping, production hosting.
3. `Windows-x64-Dev`: native diagnostic client.
4. `Windows-x64-Release`: future internal/release client.
5. `Linux-x64-Dev`: native diagnostic client.
6. `Linux-x64-Release`: future internal/release client.
7. `Linux-x64-Server`: headless authoritative server with `UNITY_SERVER`,
   presentation assemblies absent.

Every output manifest records source revision, dirty-state rejection, Unity and
Hub revisions, modules/packages, compiler/backend, build profile, scripting
defines, stripping, graphics APIs, protocol/schema/content/rules versions,
artifact hashes/sizes, build host, and reproducible command.

## Gates

This document defines the gates and their owners; it does not claim downstream
gates have passed.

### P0 — Platform contract

Owner: `mc-7e2`.

Pass when target tiers, support language, Chrome policy, provisional envelopes,
portability invariants, dependency-matrix schema, distribution/signing posture,
and downstream evidence owners are accepted.

### P1 — Toolchain and dependency admission

Owners: `mc-gzn`, `mc-9xb`, and `mc-w58.15`.

Pass when the exact editor, Hub, Web, Linux, Linux-server and scheduled Windows
modules are pinned; populated required-package rows are claim-reviewed with no
known target red state; the exact Wasm capability lock is selected; normal
vendor/tool-owned install/removal paths are specified; and supported build
hosts exist or are scheduled. No project creation or package installation
precedes this gate.

### P2 — Empty cross-platform build

Owners: `mc-u32`, `mc-ntq`, and `mc-w37`.

Pass when one revision produces empty/minimal Web, Linux engineering client,
and Linux-server artifacts; each launches on its named target; the server
contains no presentation assembly; and all publish matching schema/protocol
fingerprints. A Windows x86-64 build smoke is scheduled as soon as its supported
host exists, but public/native Windows certification remains post-demo.

### P3 — Deterministic parity

Owners: `mc-w58.9`, `mc-w58.13`, and their implementation tasks.

Pass when synthetic command vectors and a minimum replay produce matching
authoritative hashes in managed tests, headed Chrome on Windows/Linux, the
Linux native conformance client, and Linux server. Force a mismatch to prove
first-divergent-tick evidence. Add Windows-native parity before native Windows
distribution. PhysX-only equality does not satisfy this gate.

### P4 — Real hosted Chrome lifecycle

Owners: `mc-vgm`, `mc-cm9`, `mc-x8i`, `mc-7xq`, and networking gates.

Pass on the real HTTPS host in headed, hardware-accelerated current Stable
Chrome on both Tier 1 OS surfaces:

- clean load and cached load meet bootstrap/memory caps;
- production hosting successfully streams Wasm as `application/wasm` under the
  deployed CSP;
- audio activation, pointer lock, fullscreen exit, remapping, QWERTZ/AZERTY,
  and the gamepad action-adapter smoke work; polished controller certification
  is not a browser-demo blocker;
- secure realtime connection and application RTT work;
- refresh, hidden tab, focus loss, network interruption, IndexedDB denial or
  eviction, reconnect, and incompatible build are safe;
- a forced WebGL context loss/restoration recreates all GPU resources and
  resumes correctly, or performs a deliberate safe reload with explicit
  recovery;
- browser console, network trace, memory/profile capture, renderer/driver, and
  failure bundle are retained.

P4 depends on the accepted `mc-x8i` lifecycle contract. For every background or
network-stall duration, server authority continues, the client accepts no local
authoritative victory/damage/economy progress, no duplicate command is
accepted, reconnect meets the deadline frozen by `mc-x8i`, and the repaired
client hash equals the server hash before control resumes.

On Ubuntu, audio preflight verifies the AAC/GStreamer/ffmpeg prerequisite
chosen for the locked test image, activation after a user gesture, spatial
playback, and loop behavior. Missing codec support produces an explicit
requirements result rather than a silent soundtrack failure.

### P5 — Minimum-hardware technical proof

Owners: `mc-cm9`, `mc-w58.13`, and `mc-fg0`.

Pass when the exact integrated carpet/projectile/terrain/network proof runs on
the minimum Windows and Ubuntu reference floor plus the T14 quality target,
without software rendering, thermal-result ambiguity, OOM, or hidden quality
override.

### P6 — Delivery and rollback

Owners: `mc-w37`, `mc-7xq`, `mc-9xb`, and deployment tasks.

Pass when immutable delivery, correct compression/MIME/security/cache headers,
client/server version handshake, provenance/SBOM, clean-environment
reproduction, and rollback work on the real host.

Exact baseline assertions include HTTPS and WSS; `Content-Encoding: br` for
Brotli artifacts; `Content-Type: application/wasm` for Wasm; the selected CSP;
same-origin delivery or an explicit CORS allowlist; no wildcard credentialed
CORS; and JavaScript decompression fallback disabled. Any exception is a
versioned, tested hosting decision.

The delivery suite explicitly proves interrupted first install, interrupted
version update, old/new tabs open concurrently, a new worker waiting behind an
in-match client, service worker disabled/unavailable, quota failure during
cache population, partial new cache followed by offline launch, and deployment
rollback while stale clients remain open. No case may activate a mixed
code/content/protocol set.

### P7 — Playable-demo portability

Owners: `mc-w58.14` and the playable-demo integration/acceptance beads selected
by the approved execution graph.

Pass when the exact integrated demo build and server:

- complete a full match plus rematch on both Tier 1 browser surfaces;
- complete matching representative replay/hash verification through native and
  server lanes;
- meet the accepted load, memory, frame, crash, network, and recovery budgets;
- have no required platform dependency missing its matrix evidence;
- can roll back to the prior compatible deployment.

Edge, Firefox, Safari, macOS, mobile, Steam Deck, and public native installers
cannot fail this demo gate because they are not in it.

### P8 — Native distribution

Post-demo owners are selected by the release plan.

Pass separately for Windows, Ubuntu Linux, and the internal Steam Deck target
only after
clean-machine packaging, signing, update/removal, controller, suspend/resume,
performance, support, and real-device evidence. Browser success does not imply
this gate. Deck evidence additionally covers Steam Input/controller-only UI,
on-screen keyboard for text, 16:10 presentation, offline startup, and
suspend-caused network loss. Valve submission/Verified status is a separate
release decision.

## Failure artifact

Any gate failure retains:

- gate/profile and exact repro steps;
- build, source, Unity, package, schema, protocol, content, and rules IDs;
- OS/build/kernel, browser full version/channel, GPU/driver/WebGL renderer,
  display server, flags, hardware and power/thermal state;
- asset/cache state, network profile, server image/digest and topology;
- first failed invariant or divergent tick, expected/actual hash;
- browser console, player/server logs, crash dump where available, network
  trace, screenshot/video, profiler/memory/frame capture;
- workaround attempted, whether it changes product behavior, and owner/bead.

## Explicit non-goals

- Certifying every browser Unity says it can support.
- Mobile, console, VR, public native installers, stores, platform accounts, or
  cross-platform account services.
- Multithreaded Web C#, experimental native Web threads, WebGPU dependence,
  peer-to-peer authority, browser UDP/TCP, or GPU-authoritative simulation.
- Hiding an unsupported host behind unusual directories, wrappers, containers,
  emulation, or compatibility flags.
- Solving exact transport, tick rate, detailed browser lifecycle state machine,
  CI provider, hosting vendor, signing vendor, or final content budgets here;
  their named downstream beads must remain consistent with this contract.

## Sources checked

All sources were accessed 2026-07-23.

- [Unity 6 release support](https://unity.com/releases/unity-6/support):
  Unity 6.3 is the current LTS family and is supported through December 2027.
- [Unity 6.3 system requirements](https://docs.unity3d.com/6000.3/Documentation/Manual/system-requirements.html):
  named Editor/Player/server/browser OS and graphics support; Ubuntu
  22.04/24.04 Linux scope; 64-bit WebGL2/Wasm browser requirements.
- [Unity 6.3 Web technical limitations](https://docs.unity3d.com/6000.3/Documentation/Manual/webgl-technical-overview.html):
  managed-thread, timer, socket, AOT, filesystem/cache, physics, and hosting
  constraints.
- [Unity 6.3 browser compatibility](https://docs.unity3d.com/6000.3/Documentation/Manual/webgl-browsercompatibility.html):
  WebGL2/HTML5/64-bit/Wasm requirements and browser/OS matrix.
- [Unity Web memory](https://docs.unity3d.com/6000.3/Documentation/Manual/webgl-memory.html):
  contiguous/growing heap, in-memory data, IndexedDB caching, and end-of-frame
  garbage-collection constraints.
- [Unity Web networking](https://docs.unity3d.com/6000.3/Documentation/Manual/webgl-networking.html):
  Fetch/CORS, no direct TCP/UDP or `System.Net` sockets, and Web-compatible
  networking options.
- [Unity Web audio](https://docs.unity3d.com/6000.3/Documentation/Manual/webgl-audio.html):
  user-gesture/autoplay implications, Web Audio backend, positional features,
  mixer limitations, and browser decoding behavior.
- [Unity Web background execution](https://docs.unity3d.com/6000.3/Documentation/ScriptReference/Application-runInBackground.html):
  browser background update throttling and possible mobile suspension.
- [Unity Dedicated Server build](https://docs.unity3d.com/6000.3/Documentation/Manual/dedicated-server-build.html):
  supported build profiles, `UNITY_SERVER`, Linux64 server subtarget, and
  headless result.
- [Unity Dedicated Server optimizations](https://docs.unity3d.com/6000.3/Documentation/Manual/dedicated-server-optimizations.html):
  server stripping behavior and Scriptable Build Pipeline/Addressables caveat.
- [Chrome release channels](https://developer.chrome.com/docs/web-platform/chrome-release-channels):
  Stable/Beta/Dev/Canary purposes and version semantics.
- [Chrome for Testing](https://developer.chrome.com/blog/chrome-for-testing/):
  official versioned no-auto-update testing binaries and channel JSON APIs.
- [Chrome for Testing availability](https://googlechromelabs.github.io/chrome-for-testing/):
  observed Stable/Beta revisions and platform artifacts.
- [Chrome flags guidance](https://developer.chrome.com/docs/web-platform/chrome-flags):
  production validation should use Stable without experimental flags.
- [Chrome release cadence change](https://developer.chrome.com/blog/chrome-two-week-release):
  Stable/Beta move to a two-week cadence starting September 2026; Extended
  Stable remains on its existing cadence.
- [Ubuntu release cycle](https://ubuntu.com/about/release-cycle):
  Ubuntu 24.04 LTS remains under standard maintenance through May 2029.
- [Windows 11 release information](https://learn.microsoft.com/en-us/windows/release-health/windows11-release-information):
  current supported Windows 11 versions/builds and lifecycle.
- Chrome system requirements for
  [Windows](https://support.google.com/chrome/a/answer/7100626) and
  [Linux](https://support.google.com/chrome/a/answer/9025903):
  official browser floors; this project deliberately certifies a narrower
  supported Windows 11/Ubuntu 24.04 set.
- [Chromium SwiftShader documentation](https://chromium.googlesource.com/chromium/src/%2B/1330866/docs/gpu/swiftshader.md):
  software-rendered WebGL is distinct from real-GPU evidence.
- [MDN canvas context creation](https://developer.mozilla.org/en-US/docs/Web/API/HTMLCanvasElement/getContext)
  and [WebAssembly validation](https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/JavaScript_interface/validate_static):
  capability-driven WebGL2 and Wasm startup checks.
- [MDN storage quotas and eviction](https://developer.mozilla.org/en-US/docs/Web/API/Storage_API/Storage_quotas_and_eviction_criteria)
  and [Service Worker API](https://developer.mozilla.org/en-US/docs/Web/API/Service_Worker_API):
  cache eviction, quota failure, HTTPS, and update-lifecycle considerations.
- [Steam Deck hardware](https://www.steamdeck.com/en/hardware),
  [Valve Deck compatibility](https://partner.steamgames.com/doc/steamhardware/compat),
  and [Proton documentation](https://partner.steamgames.com/doc/steamhardware/proton):
  device architecture/resolution and native-Linux versus Proton acceptance
  paths.
