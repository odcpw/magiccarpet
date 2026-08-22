# Cross-platform targets, portability rules, and build gates

> **Historical/subordinate as of 2026-08-21.** Its two-track/C#/browser premises
> are retired. Current platform order and authority live in
> `stack-decision.md`; retain only measured portability constraints.

Version: 1.0  
Status: historical portability research
Governing documents: `program-charter.md`, `stack-decision.md`

## Decision

Track `P` and Track `S` have different source constraints and therefore
different build matrices.

- `P`: native remc2/MC2HD first; bounded C++/Emscripten owner-local browser
  proof second.
- `S`: Unity 6 LTS/URP is the initial measured baseline for Chrome, native,
  Linux authority and later OpenXR. A browser-native stack is a fallback only
  if Unity fails declared hard gates after bounded remediation.

No Track P C++ binary, library, Wasm module, source, state layout or asset
enters Track S.

## Track P matrix

| Target | Priority | Purpose |
|---|---:|---|
| Linux x86-64 native | P0 | Reproducible development and regression |
| Windows x86-64 native | P0 | Legion/Binary Ninja and release comparison |
| Linux headless test harness | P1 | Step/regression feasibility |
| Chrome Wasm owner-local | P1 experiment | Technical preservation proof |
| Public browser/installer | Blocked | Rights, branding, data and release gate |

The first Wasm slice:

- builds remc2 only, not configurator/editor/regression executables;
- disables native networking and multithreaded rendering;
- provides data through browser-local virtual storage;
- converts blocking outer-loop ownership to a browser-compatible step/main
  loop;
- uses browser audio/input/filesystem adapters;
- treats Asyncify as bring-up only unless measured overhead is accepted; and
- compares native and Wasm deterministic checkpoints.

## Track S matrix

| Target | Tier | Baseline |
|---|---:|---|
| Chrome on supported Windows/Linux | 1 | Unity Web, WebGL2 baseline; optional WebGPU observation |
| Linux authority | 1 | Unity Dedicated Server or pure managed host selected by proof |
| Windows/Linux native | 2 | Same simulation/content identity |
| Steam Deck-class Linux | 2 | Native/controller quality gate |
| Native PC VR | Later | Unity OpenXR presentation/input adapter |
| Browser VR, mobile, console | Non-target | Separate future decision |

## Successor portability laws

1. Authoritative simulation has no rendering, audio, camera, UI, editor,
   wall-clock, ambient RNG, nondeterministic iteration or PhysX-result
   dependency.
2. Presentation can vary by quality/platform without changing authoritative
   state or information eligibility.
3. Chrome, native, replay and server run the same commands, rules identity,
   serialization and state-hash contract.
4. Browser networking uses a secure browser-supported transport; it never
   assumes raw TCP/UDP.
5. Browser storage/cache is recoverable acceleration, not sole authority.
6. Hidden/background browser state is treated as a stall/disconnect risk.
7. Audio begins after user gesture and degrades explicitly when unsupported.
8. Native plug-ins and Rust/C++ FFI are excluded from the Unity Web baseline.
9. Assets are content-addressed, budgeted, provenance-manifested and validated
   before inclusion.
10. Unsupported systems receive an exact preflight failure rather than crash
    or indefinite load.

## Logical boundaries

Provisional Unity names:

```text
Successor.Contracts
Successor.Determinism
Successor.Simulation
Successor.Game
Successor.Persistence
Successor.Network.Protocol
Successor.Client.Unity
Successor.Client.WebBridge
Successor.Server
Successor.Replay
Successor.Tools
```

`Successor.Simulation` and `Successor.Game` use plain deterministic C# unless
the stack gate chooses a different successor core. `UnityEngine` belongs only
in client/server host adapters explicitly admitted by the contract.

TypeScript owns the surrounding web page, service worker/cache, room/lobby
flow, capability diagnostics and JS transport bridge. It does not own
authoritative simulation.

Python/Blender owns deterministic asset generation/export/validation. Rust may
own offline city ingestion, data transformation, provenance and packaging
tools where it improves safety/performance. Rust does not enter the Unity Web
runtime by default.

## Successor browser budgets

Hard S0 gates, measured on exact pinned release builds:

- compressed engine/bootstrap `<= 50 MiB`;
- initial controllable slice `<= 100 MiB`;
- cold link-to-control `<= 30 s` at 50 Mbps and 50 ms RTT;
- cached link-to-control `<= 10 s`;
- minimum 720p frame time p99 `<= 33.3 ms` and p99.9 `<= 50 ms`;
- browser process steady memory `<= 1.5 GiB` and peak `<= 2 GiB`;
- retained growth `<= 5%` across full match plus rematch;
- canonical simulation step p99 `<= 4 ms` on minimum client hardware;
- no post-load main-thread stall over `250 ms`;
- one-room server memory `<= 1 GiB`, 30 Hz p99 tick `<= 16.7 ms`, and p99.9
  `<= 25 ms` on two vCPUs;
- structured diagnostics for browser, GPU, memory, asset, network and server
  state.

The reference workload, network thresholds, fallback triggers and remediation
limits are governed by
[`successor-stack-pin.md`](successor-stack-pin.md). They cannot be silently
waived.

## Stack proof gates

### S0.1 Toolchain and agent loop

- install through normal supported paths;
- pin exact Unity/CLI/MCP/package/Blender revisions;
- create/open/build/test from CLI;
- manipulate and inspect a synthetic scene through MCP;
- recover from compile/editor/runtime failures;
- reproduce from a clean checkout.

### S0.2 Empty artifacts

- Chrome;
- Windows/Linux native;
- Linux server;
- deterministic test/replay runner.

### S0.3 Common technical scene

Run the exact workload specified in `stack-decision.md`, including four Chrome
clients, Blender assets, terrain edit, weather/audio/HUD and replay hashes.

### S0.4 Real hosted lifecycle

HTTPS, correct headers, cache invalidation, content hashes, room join,
reconnect, hidden tab, audio gesture, corrupt/partial cache, rollback and
failure bundle.

### S0.5 Decision

Accept Unity; accept with bounded remediation; or trigger the browser-native
fallback. Do not maintain two complete client stacks speculatively.

## Failure artifact

Every failed gate records source/stack/package IDs, OS, browser, GPU/driver,
build profile, content/rules hashes, command/seed, logs, browser console,
memory/profile capture, network trace, state hashes and first divergence,
without private Track P evidence or personal data.
