# Successor stack pin and proof contract

Version: 1.0  
Status: planning pin; installation requires `mc-w58.15` approval  
Verified: 2026-07-23  
Track: `S` only  
Decision owner: Oliver

## Outcome

The first original successor proof uses Unity as the client, editor and content
toolchain. It does not make Unity scene objects, PhysX, Netcode for GameObjects
or an editor-control tool the authority for the game.

```text
Unity 6000.3.20f1 + URP 17.3.0
  presentation, input, audio, UI, profiling and content authoring
              |
              v
Unity-free deterministic C# simulation and protocol
  integer/fixed-point or explicitly quantized canonical state
              |
              +--> Unity Web client over WSS
              +--> native Unity client
              +--> Unity Dedicated Server candidate
              `--> .NET 10 headless host candidate
```

The browser release gate is current stable 64-bit desktop Chrome on supported
Windows and Linux. WebGL 2/Wasm is the required renderer/runtime. WebGPU is an
observation lane, not a rescue condition or baseline.

This record pins what can be pinned before project creation. Editor-coupled
package patches are accepted only after the approved editor creates the empty
project, UPM resolves them without warnings, and both `manifest.json` and
`packages-lock.json` are captured. No `latest`, floating branch, unreviewed
package update or direct dependency on an editor-control server is allowed.

## Exact tool and package matrix

| Layer | Planning pin | Disposition |
|---|---|---|
| Unity Editor | `6000.3.20f1`; changeset `c9ba695d4f07`; 2026-07-16 | Exact LTS patch for Linux and Legion |
| Unity modules | Web Build Support; Linux Dedicated Server Build Support | Install only these plus editor-required dependencies for S0 |
| Project template | `com.unity.template.urp-blank` `17.1.0`; SHA-1 `f8b904705840d6bb96bf4f9bce1676e9190f7929` | Live Unity CLI metadata for `6000.3.20f1`; lock the resolved project rather than the CLI's older bundled fallback |
| Render pipeline | `com.unity.render-pipelines.universal` `17.3.0` | Exact built-in package selected by `6000.3.20f1` UPM; the live template declares a `17.0.4` floor, but the resolved project lock is authoritative |
| Input | `com.unity.inputsystem` `1.14.2` | Exact dependency declared by the live template; baseline action maps |
| Tests | `com.unity.test-framework` `1.6.0` | Exact dependency declared by the live template; EditMode, PlayMode and player smoke tests |
| Transport | `com.unity.transport` `2.7.4` | Baseline WebSocket/UDP transport adapter |
| NGO | `com.unity.netcode.gameobjects` `2.13.0` | Bounded adapter candidate; never canonical state |
| Multiplayer tools | `com.unity.multiplayer.tools` `2.2.9` | Profiler and network simulator |
| Multiplayer Play Mode | `com.unity.multiplayer.playmode` `2.0.2` | Legion only; it is not officially supported on Linux |
| Unity CLI | `0.1.0-beta.7`; 2026-06-16 | Experimental convenience; direct Editor batch mode remains the floor |
| Unity Pipeline | `com.unity.pipeline` `0.3.1-exp.1`; registry SHA-1 `58c16695e488f3b514078778bb94bfca22c9379d` | Optional official editor-control path; exact lock required |
| Coplay MCP for Unity | `v10.1.0`; commit `c14de1e6dc01ab42d2bb358730cff954bce0ce6b`; MIT | Optional local editor authoring path; excluded from runtime and builds |
| Coplay Python lock | `Server/uv.lock`; repository SHA-256 `43077a0f35d2a7c8cc0f4731c53dd239491577288f9d8ed35fd7d59414881819` | Install from the pinned repository revision and preserve the lock |
| Blender | `4.5.12 LTS`; 2026-07-21 | Headless `bpy` export in an isolated profile |
| Blender Linux archive | `blender-4.5.12-linux-x64.tar.xz`; SHA-256 `95e3a2dfedba3bd32ca54fc355eac6b15a11986954ccb02815a07535d0120a25` | Vendor archive or supported system package in a normal install location |
| Pure managed server candidate | .NET SDK `10.0.302`; runtime `10.0.10` LTS | Server bakeoff only; not required in the Unity Web player |
| Native XR, deferred | XR Plug-in Management `4.7.0`; OpenXR `1.17.1` stable | Install only when the native VR milestone starts |

`com.unity.services.multiplayer` and Addressables are deliberately absent from
S0. A direct WSS browser-to-authority proof comes first. Cloud lobby, relay,
matchmaking and streamed-content services are considered only after their
coupling, price, quotas, data flow and failure modes are justified.

The exact package graph accepted by S0 is the resolved lock, not this table in
isolation. The provisioning gate must record package tarball hashes, registry
source, transitive versions and license/notice material.

## Canonical simulation rules

“Pure C#” alone is not a determinism guarantee. Canonical position, velocity,
time, terrain values, resource custody, health, damage, identifiers and random
state use integers, fixed point or precisely specified quantization.

Canonical code must not depend on:

- `UnityEngine`, scene traversal, `MonoBehaviour`, PhysX result ordering or
  Unity frame time;
- unquantized `float` or target-specific transcendental math;
- unordered collection enumeration;
- wall-clock time, timers or ambient randomness;
- reflection-generated or target-dependent serialization;
- rendering, audio, camera, UI, browser state or network arrival order.

The command stream, snapshot bytes, canonical state bytes and state hashes are
versioned. The same test corpus runs under Editor/.NET, native IL2CPP, browser
IL2CPP/Wasm and both server candidates.

Hard determinism acceptance is zero divergence across at least 1,000,000
simulated ticks over ten declared seeds, including snapshot/restore and
reconnect-tail replay. The first response to failure is to repair the numeric,
iteration or serialization contract. A Rust native/Wasm kernel is considered
only if the integer/fixed implementation still diverges after two focused
remediation days and a minimal reproducer demonstrates why.

## Server decision

Unity remains the client/editor regardless of which headless host wins.

Run the same Unity-free simulation and versioned protocol in:

1. Unity 6000.3 Dedicated Server for Linux; and
2. a .NET 10 Linux host.

Prefer the .NET host if it runs the complete authority without Unity services,
scenes, assets or physics and is simpler to deploy and observe. Retain Unity
Dedicated Server only if it materially reduces integration work without
contaminating the simulation boundary.

Server hard gate for the synthetic S0 match:

- 30 Hz authority;
- p99 tick at or below `16.7 ms` and p99.9 at or below `25 ms` on two vCPUs;
- no GPU;
- cold readiness at or below five seconds;
- one room at or below `1 GiB`;
- later operational target of four rooms within `2 GiB`;
- no unbounded retained growth through match, rematch and reconnect.

A server-candidate failure selects the other server candidate. It does not
trigger replacing Unity as client/editor.

## Protocol and network pin

Use a versioned custom command/snapshot/repair protocol over a swappable
transport interface:

- Chrome: Unity Transport WebSocket client over production `wss://`;
- native and server: WebSocket first for parity; UDP can be a separately
  measured native lane;
- browser clients never host or become canonical authority;
- TLS termination is ordinary deployment infrastructure, not simulation code;
- NetworkObject or `MonoBehaviour` replication may assist presentation but
  never defines canonical state;
- large snapshots and repair data are chunked and separated from
  latency-sensitive command traffic.

Network hard gate:

- four clients under `120 ms` RTT, `20 ms` jitter and `1%` packet loss;
- average at or below `256 kbit/s` per client and p95 at or below
  `512 kbit/s`;
- hidden-tab restoration and repair within five seconds;
- checkpoint/tail reconnect within ten seconds;
- no duplicated command, resource transfer, impact, objective or result.

If WebSocket head-of-line blocking fails, first test separate control/bulk
WebSockets or a bounded JavaScript/WebTransport adapter. That is a transport
decision, not an engine-fallback trigger.

## Browser hard gates

These replace the earlier permissive `250 MiB`/`120 s` planning allowance.
Measure release builds on the declared reference connection and minimum
hardware, with a clean profile and a warm-cache repeat.

| Area | Hard pass |
|---|---:|
| Compressed engine/bootstrap | `<= 50 MiB` |
| Initial controllable slice, all requests | `<= 100 MiB` |
| Cold link-to-control at 50 Mbps / 50 ms RTT | `<= 30 s` |
| Cached link-to-control | `<= 10 s` |
| 720p minimum frame time | p99 `<= 33.3 ms`; p99.9 `<= 50 ms` |
| Browser process memory | steady `<= 1.5 GiB`; peak `<= 2 GiB` |
| Retained growth | `<= 5%` across match plus rematch |
| Canonical simulation step | p99 `<= 4 ms` on minimum client hardware |
| Post-load main-thread stall | none over `250 ms` |

The representative proof includes flight, projectile, terrain query/edit,
100 actors plus the declared burst, resource ledger/objective, original
Blender static and rigged proxies, water, vegetation, volcano, storm,
day/dusk, spatial audio, HUD/minimap, four clients and replay/hash evidence.
No renderer candidate may substitute an easier workload.

Hard browser fallback is triggered only after two bounded optimization passes
when the representative release still:

- exceeds `100 MiB` or `60 s` link-to-control;
- exceeds `50 ms` p99 at the 720p minimum preset; or
- exceeds `2 GiB`, crashes or cannot complete match plus rematch.

The first challenger is a separately scoped Three.js/WebGPU-with-WebGL2-
fallback presentation using the same simulation, protocol, assets and
workload. Do not build it speculatively.

## Agent and editor automation

There are three replaceable layers:

1. direct Unity Editor batch mode and checked-in `executeMethod` scripts;
2. official experimental Unity CLI plus Pipeline;
3. pinned Coplay MCP for high-level scene and importer interaction.

Every important mutation must be inspectable in source/assets and every build
or test must have a direct batch-mode command. A CLI or MCP failure removes or
repairs that tool; it is not evidence that Unity failed.

Tooling proof:

- 20 consecutive edit/build/test cycles;
- p95 code-change-to-focused-test at or below three minutes;
- scene mutation plus validation at or below two minutes;
- forced compile-error recovery within five minutes;
- Editor restart recovery within five minutes;
- zero corrupt scenes or assets;
- all builds and tests still work with MCP disabled.

### Coplay MCP controls

Coplay MCP is broad write-capable editor automation. `execute_code` explicitly
runs arbitrary C# in the Editor and its own source says its pattern checks are
not a sandbox. The `asset_gen` group can transmit prompts and inputs to
third-party model providers.

For S0:

- use tag `v10.1.0` at the recorded commit, never `main`;
- install with the repository `Server/uv.lock`;
- use HTTP local mode bound to `127.0.0.1`, not LAN or remote-hosted mode;
- set `UNITY_MCP_DISABLE_TELEMETRY=1`;
- keep `scripting_ext`, `asset_gen` and custom project tools disabled;
- do not configure provider API keys;
- do not use remote API-key mode;
- target only the separate successor project, never this private preservation
  lab;
- inspect the checkout diff after every material MCP mutation;
- verify process/port cleanup across compile reload and Editor restart.

Do not use stdio as the approved baseline unless its startup visibility is
fixed or independently guarded. At `v10.1.0`, the server registers all tool
groups at stdio startup before it synchronizes Editor toggles; local HTTP mode
starts non-default groups disabled and is easier to audit.

## Blender and asset contract

`.blend` is authored source and never placed directly under Unity `Assets`.

```text
.blend source
  -> Blender 4.5.12 headless + pinned bpy script
  -> reviewed FBX baseline
  -> semantic validation and provenance manifest
  -> Unity import settings
  -> runtime artifact
```

The export profile fixes units, axes, transforms, modifier policy, skeleton,
clip ranges, material slots, texture color spaces, generated names and tool
versions. Validation hashes normalized semantic content—geometry,
indices, materials, skeleton, clips, bounds and provenance. It does not demand
byte-identical FBX containers when timestamps or exporter metadata differ.

GLB/glTF remains a portable challenger and requires an explicitly pinned,
licensed Unity importer. It does not block the FBX baseline.

## Web and XR constraints

Architecture and tests must reflect that Unity Web:

- uses IL2CPP/AOT and WebAssembly;
- runs managed C# on one thread;
- has no raw TCP/UDP or supported `System.Net`;
- has restricted Web Audio and requires a user gesture;
- can be suspended or throttled in a hidden tab;
- has explicit memory and asset-residency pressure;
- cannot assume cross-target PhysX identity;
- does not directly support XR.

Native OpenXR is a later input/presentation adapter. XR packages do not enter
the browser proof or S0 package graph.

## Installation and evidence commands

These commands are the approved shape, not evidence that installation has
already happened. Provisioning must discover current module IDs before
installing and must fail on a version mismatch.

```bash
# Official CLI: use the vendor installer and beta channel, then verify exactly.
curl -fsSL https://public-cdn.cloud.unity3d.com/hub/prod/cli/install.sh \
  | UNITY_CLI_CHANNEL=beta bash
unity --version
unity releases --format json
unity modules list 6000.3.20f1 --format json

# Install the exact editor and only the approved modules after mc-w58.15.
unity install 6000.3.20f1 --dry-run --format json
unity install 6000.3.20f1 --accept-eula --non-interactive
unity install-modules -e 6000.3.20f1 -m <verified-web-module> <verified-linux-server-module>
unity editors info 6000.3.20f1 --format json
unity license status --format json

# Official Pipeline; reject and pin any result other than 0.3.1-exp.1.
unity pipeline install --project-path <successor-project>
unity pipeline list --format json

# Direct Editor floor. Resolve the standard editor path; do not use a wrapper.
<Unity> -batchmode -quit -projectPath <successor-project> \
  -executeMethod Successor.Tools.Build.VerifyEnvironment -logFile <log>
<Unity> -batchmode -quit -projectPath <successor-project> \
  -runTests -testPlatform EditMode -testResults <results.xml> -logFile <log>
<Unity> -batchmode -quit -projectPath <successor-project> \
  -executeMethod Successor.Tools.Build.BuildWeb -logFile <log>
<Unity> -batchmode -quit -projectPath <successor-project> \
  -buildTarget Linux64 -standaloneBuildSubtarget Server \
  -executeMethod Successor.Tools.Build.BuildLinuxServer -logFile <log>

# Blender evidence after supported installation.
blender --version
blender --background --factory-startup --python <export-script> -- \
  --input <source.blend> --output <artifact.fbx> --manifest <manifest.json>
```

Store:

- editor version, changeset, module installer provenance and license state;
- `ProjectVersion.txt`, `manifest.json`, `packages-lock.json` and their hashes;
- Unity CLI/Pipeline versions and structured diagnostics;
- Coplay tag, commit, `uv.lock` hash, configuration, enabled tool list and
  telemetry status;
- Blender version, archive checksum, command and semantic manifest;
- exact commands, exit codes, full logs and rollback instructions.

Use standard vendor/tool-owned locations. If installation requires
administrator access, ask Oliver for `sudo` or use the normal system
authorization prompt. Do not create alternate prefixes or PATH wrappers.

## Retain/change rule

Retain Unity when the exact common proof passes browser, determinism,
networking, server, asset and iteration gates.

The decision ladder is:

1. fix the local code/configuration defect;
2. make at most two focused optimization passes for a failed technical gate;
3. replace a failed CLI, MCP, transport or server adapter without replacing
   Unity;
4. invoke a separately scoped browser renderer challenger only for a measured
   browser failure;
5. consider a Rust kernel only for a reproduced numeric/performance failure;
6. consider another full engine only if both the Unity client and the bounded
   browser-native challenger fail for named reasons.

No polishing system may be added to make a failing benchmark look acceptable.
Budget 60–80 focused hours and approximately 15 working days for the complete
S0 proof and signed keep/change decision.

## Primary sources

- [Unity 6000.3.20f1 release and modules](https://unity.com/releases/editor/whats-new/6000.3.20f1)
- [Unity 6 support policy](https://unity.com/releases/unity-6/support)
- [Unity CLI introduction](https://docs.unity.com/en-us/unity-cli)
- [Unity CLI release notes](https://docs.unity.com/en-us/unity-cli/release-notes)
- [Unity Pipeline package](https://docs.unity.com/en-us/unity-production-pipeline/local-tools-cli/unity-pipeline-package)
- [Unity package registry](https://packages.unity.com)
- [Unity Web technical overview](https://docs.unity3d.com/6000.3/Documentation/Manual/webgl-technical-overview.html)
- [Unity Web networking](https://docs.unity3d.com/6000.3/Documentation/Manual/webgl-networking.html)
- [Unity Web memory](https://docs.unity3d.com/6000.3/Documentation/Manual/webgl-memory.html)
- [Unity multiplayer overview](https://docs.unity3d.com/6000.3/Documentation/Manual/multiplayer-overview.html)
- [Unity Dedicated Server build](https://docs.unity3d.com/6000.3/Documentation/Manual/dedicated-server-build.html)
- [Unity model format guidance](https://docs.unity3d.com/6000.3/Documentation/Manual/3D-formats.html)
- [Unity Personal and current terms](https://unity.com/products/unity-personal)
- [Coplay MCP for Unity v10.1.0](https://github.com/CoplayDev/unity-mcp/releases/tag/v10.1.0)
- [Blender 4.5 LTS downloads](https://download.blender.org/release/Blender4.5/)
- [.NET 10 downloads and support](https://dotnet.microsoft.com/en-us/download/dotnet/10.0)
