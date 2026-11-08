# Shared presentation boundary

Rust owns the game and session lifecycle, commands, gameplay state, RNG, terrain
edits, saves and replay. Presentation reads committed state and submits input.
Modern and Voxel use the same session; changing the renderer does not create a
second simulation or change the definition/content ownership described in
[ENGINE.md](../../../ENGINE.md).

## Hot state delivery

The shared encoder is in sim-session/src/render_publication.rs. Its
[MCPO v1 wire contract](../remc2-sim-rs/crates/sim-session/RENDER_PUBLICATION.md)
has explicit little-endian scalar fields, UTF-8 strings, version and length.
It is independent of Unity, native pointers and browser APIs.

The C ABI render-observation copy acquires one live runtime borrow. It validates
the requested runtime generation, session generation and committed tick, then
writes identity, terrain revision and the complete campaign render view together.
It does not compute diagnostic state/terrain/journal hashes or refresh the
legacy cloned presentation adapter. An undersized buffer is left unchanged and
reports the required length. No borrowed state pointer escapes.

Unity's instance-owned CampaignRenderTransport reuses its byte buffer and
decodes a complete packet. GameRuntimeTransport.CampaignAdvance submits one
command and obtains its completed publication. Cameras, spells, UI, audio joins
and Modern/Voxel composition reuse that observation between simulation steps.
The transport retains validated definition/runtime metadata; guarded
pause/resume/reset/load/preferences/mission transitions refresh it explicitly.
Paused presentation can change at the same tick and is read after its own
service call. An observation failure cannot retry an already accepted command.

Buildings and existing actor/voxel observations use bounded typed copy APIs.
Consumers check their generation and tick against the completed publication.
Terrain meshes change by revision; unchanged terrain is not repeatedly hashed
to determine whether it changed.

## Events and control operations

A state publication is replaceable; ordered effects, deaths, impacts, audio
receipts and other journals are not. Their existing cursor/retention checks
remain authoritative. Seeing a newer pose must not silently skip transient
events or replay historical explosions after a load.

JSON remains for definition/configuration data, full diagnostic snapshots,
replay persistence, small existing command/control interfaces and some journal
APIs. Removing text from every interface is not the performance objective.
The frequent bulk state path must avoid repeated diagnostic work and polling.
The additive completion-result API reports a no-op without fetching full
runtime identity JSON.

The -mc-render-json measurement switch selects the preceding compact JSON path;
-mc-render-diagnostics additionally requests the full diagnostic snapshot.
These are verification controls, not separate game modes. They must preserve
the admitted command stream and simulation speed.

## Browser and future renderers

The web/src/render-publication.ts reader consumes the same packet through
DataView, preserving u64 counters and actor identities as bigint. Its live
conformance test compares every decoded field against a real native Level 1
session's diagnostic projection. Both readers bound packet size to 16 MiB and
array counts to 65,536 and reject unsupported or malformed data.

The shared encoder is independent of the native ABI. A future campaign WASM
adapter will pump the same Rust session and expose a completed packet through
linear memory. The browser must reacquire memory views after growth and finish
reading/copying a publication before its storage is reused. The current sim-wasm
flight demo is not the full campaign, and the experimental Three.js native-server
client is not a completed WASM Level 1 client. Those integrations and visual/gameplay
parity remain subsequent work.

Simulation still runs synchronously in the Unity client. A dedicated simulation
owner and asynchronous mesh/collider generation require their own measured
changes, with input ordering, pause/save/load and event retention preserved.

## Installed qualification, 2026-11-08

Normal run-modern.sh and run-voxels.sh use the same qualified
Builds/Presentation player. The launcher pins all 170 payload files, the
owner-local source building layout and replay sidecars. The actual Level 1 start
is the default; --goat and --archer select the preserved earned-input scenarios.
F5 saves the current native replay. Explicit previous voxel selectors and the
frozen environment sandbox remain available. Existing old replays were not
rewritten or admitted by weakening validation.

The private evidence root is generated/mc-engine-benchmark-20261108. Native
dc5ea14833386b7831c90d6148b06eb6dd949c0df683dd3c0a37e7b01bd91216
was built from an immutable 941-input source closure, including concurrent
native-family work, with before/after manifest agreement. The installed C#
assembly uses 340 pinned source/import inputs and 258 unchanged resource files.
The snapshot and receipt describe that complete source state; a Git revision
alone does not describe other owners' concurrent uncommitted changes.

Fourteen focused Unity checks passed, along with native workspace/all-target,
binary lifecycle, diagnostic parity and C11 header checks. Four TypeScript
checks include real native-to-browser-reader field parity at two Level 1
boundaries. The shared sim-session codec compiles for wasm32-unknown-unknown;
this is not full campaign execution in a browser.

Real input checks cover view switches, pause/resume, casting, a visible
projectile/impact lifetime, house destruction, save and clean exit. Fresh
installed loading restores all 626 removed house cells at content hash
15883481164168150708, with native and visual counts equal and no historical
explosion emission. Installed Modern advances real Rust ticks, renders all 32
source buildings and has zero full identity JSON reads in its observed window.
Opening and post-impact frames were inspected.

Three interleaved pairs used the same executable, native library, replay,
camera, assets and 1280x960 output on a private RTX 3090 GPU compositor, without
image readback. Each measured native ticks 31–361 after warmup:

| Same-build read path | Median average FPS | Median p95 frame time | Native ticks/s |
| --- | ---: | ---: | ---: |
| Former full diagnostic and identity polling pattern | 47.37 | 31.61 ms | 29.33 |
| Binary publication and metadata reuse | 223.16 | 16.46 ms | 29.91 |

These numbers measure removal of redundant polling, hashing and serialization
together, not the binary encoding alone. Control runs made roughly 12,900 full
identity reads each; publication runs made zero. Native publication copy and
managed decode medians were 0.132 and 0.038 ms. Initial loading still stalls for
about 2.3 seconds, and warm maximum frames reached 37–58 ms in the candidate.
The synchronous simulation and destruction work are not thereby qualified.
Full gameplay and matched Modern/Voxel/Three.js-WASM benchmarks remain next work.

The prior intermittent music WAVE integrity failure remains unexplained.
Mismatch diagnostics now retain the actual digest, repeated digest, fresh
reread and path while preserving the original failure. Successful installed
runs do not establish that this older issue is fixed. Older integration
attempts rejected obsolete collision-bank fixtures before gameplay; the current
installed-package lifecycle checks passed without weakening those validations.
C# UBS scanning remains partial because legacy callable/null-flow analysis is
incomplete; no candidate critical finding was established.
