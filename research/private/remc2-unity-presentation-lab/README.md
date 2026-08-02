# Unity presentation lab

This is the isolated Unity project for the optional `classic-hd` and
`unity-enhanced` presentation modes. It is deliberately separate from
`remc2-unity-native`, which remains the sealed Exact project.

The lab now provides the safe shell, the owner-bound Classic HD renderer
binding, and the owner-bound Unity Enhanced presentation binding under
qualification:

- Unity `6000.3.20f1` with URP `17.3.0`.
- A `Boot` scene with a pre-session mode router.
- External owner-input validation for package, closure, and asset-cache paths.
- A diagnostic `NO OWNER INPUTS` boot that never opens a session.
- A deterministic presentation-session lease that does not own gameplay or
  load owner payloads.
- A source-style Classic HD route with a persistent dimensioned R8 index
  surface, one top-row-to-Unity-row upload per frame, resolved night palette,
  point/bilinear output filtering, and a centered 4:3 display rectangle.
- A size-queried `sim-capi` binding contract. The player-facing Classic HD
  route opens the external owner package, closure, and cache through the Rust
  native session and calls the additive profile-sized Classic HD rerender
  endpoint directly into the persistent HD R8 surface. The profile-sized
  source compositor retains source order and the existing family-specific
  clocks, banks, reflection, and shadow rules for typed and legacy families. It
  never enlarges the 320x200 Exact FINAL MAIN. The route still fails closed
  when the native plugin is absent or a future source family is unbound; the
  current Level 1 owner smoke proves the 640x480 idle frame and upload, and
  the sequence smoke proves 32-frame rerender/upload plus reset replay without
  surface reallocation.
  The additive Classic HD ABI fixture remains available for contract tests,
  while the direct component `Configure` overload remains a clearly
  synthetic, owner-free Level 1 Night source for EditMode tests.
- JSON settings outside owner data with fail-closed schema/profile validation.
- A procedural Unity Enhanced stack over the same snapshot boundary:
  authoritative height/type/shade/angle terrain planes with generated normals
  and dirty-region updates; pooled generated visuals for the complete Level 1
  Night actor/family coverage; descriptor-driven moon/fog/local-light caps;
  procedural water and shoreline/reflection fallback; ordered replayable spell,
  mana, destruction, weather, and speed VFX; and URP HDR/volume/post controls.
- Enhanced quality tiers `low`, `medium`, `high`, and `whole-hog`, reduced
  motion controls, frame/pool/drop diagnostics, and explicit spatial render
  scale choices (`1.0`, `0.85`, `0.75`). Temporal upscaling is gated off by
  default and cannot be enabled accidentally by a quality preset.
- The owner-bound Unity Enhanced route opens the external package, closure, and
  asset cache through the existing Rust `sim-capi` v2 session, validates the
  presentation-camera history and live terrain/entity records, and applies
  immutable generation-bound snapshots to the URP-only hierarchy. Unity owns
  only generated meshes, lights, effects, camera interpolation, and pooling;
  Rust remains gameplay authority. The router applies a separate generation
  reset snapshot at tick 0, publishes sequence-bound input in `Update`, and
  advances one owner step per fixed simulation tick; unchanged terrain is not
  rebuilt, while map-wrap epoch changes force a complete rebase. The
  deterministic owner sequence smoke covers flight input, spell
  selection/cast, 32-frame reset replay, terrain/water/night lighting, native
  hash correspondence, and the Rust-owned event/mission-effect seam: the
  existing event journal and Level 1 projectile/impact rows are validated at
  their generation/tick boundary, drive generation-bound pooled actor views and
  capped local lights, and drive pooled Enhanced VFX without embedding owner
  payloads. Inactive dynamic rows recycle through the same Unity presentation
  pool, while Rust remains the source of identity, position, kind, and lifetime.
  The same smoke pins the action-51 terrain qualification vector (house 11,
  parent 152, selector 17): every observed production mutation causes one
  terrain rebuild, and a settled unchanged step causes none.
- The owner-bound Unity Enhanced scenario smoke extends that bridge through
  eight idle frames, a moving town route, a Fireball combat route with one
  projectile and impact row, high-speed flight, lethal player state, and a
  fresh tick-0 reset generation. Its receipt is
  `research/private/remc2-presentation/qualification/unity-enhanced-owner-route-scenarios.json`;
  completion/postgame, window/alt-tab, owner/GPU, and visual-corpus gates stay
  separate until their player-facing evidence exists.
- No copied owner assets, native libraries, Exact scripts, or Exact corpus.

Run the editor setup once after opening the project:

```text
Unity -batchmode -nographics -quit \
  -projectPath research/private/remc2-unity-presentation-lab \
  -executeMethod MagicCarpet.PresentationLab.Editor.CreatePresentationLabAssets.CreateRenderPipelineAssets \
  -logFile /tmp/magiccarpet-presentation-lab-setup.log
```

The runtime accepts `-mc-presentation-mode classic-hd` or
`-mc-presentation-mode unity-enhanced`. The owner inputs remain external:

```text
-mc-package /absolute/path/to/level1-export-m \
-mc-closure /absolute/path/to/level1-asset-closure.json \
-mc-asset-cache /absolute/path/to/level1-unity-asset-cache
```

Classic HD defaults to internal `640x480` and can be overridden before scene
assembly with `-mc-settings /absolute/path/to/classic-hd-settings.json` or
the equivalent flags (`-mc-classic-profile`, `-mc-classic-asset-tier`,
`-mc-classic-view-distance`, `-mc-classic-reflections`,
`-mc-classic-dynamic-lighting`, `-mc-classic-blur`, `-mc-ui-scale`,
`-mc-fullscreen`, `-mc-window-width`, `-mc-window-height`, and
`-mc-output-filter`). The settings file lives under Unity's
`Application.persistentDataPath` when no explicit path is given; it never
uses the owner package, closure, or asset cache as a settings store.

The private three-mode launcher is
`launcher/presentation_launcher.py`. Its checked-in config defaults to the
sealed Exact Built-in project, emits no mode switch for Exact, and points
Classic HD and Unity Enhanced at this isolated lab only when all three owner
inputs are supplied explicitly:

```text
python3 launcher/presentation_launcher.py --validate
python3 launcher/presentation_launcher.py --mode classic-hd \
  --owner-package /absolute/path/to/level1-export-m \
  --owner-closure /absolute/path/to/level1-asset-closure.json \
  --owner-cache /absolute/path/to/level1-unity-asset-cache
```

The lab does not accept `exact`; Exact stays in the protected project. Mode
selection happens before a presentation session opens, and there is no
mid-session pipeline swap.

The owner-bound Classic HD sequence smoke is an explicit qualification command
rather than an automatic boot action:

```text
Unity -batchmode -force-glcore \
  -projectPath research/private/remc2-unity-presentation-lab \
  -executeMethod MagicCarpet.PresentationLab.Editor.CreatePresentationLabAssets.RunOwnerClassicHdSequenceSmoke \
  -mc-presentation-mode classic-hd \
  -mc-package /absolute/path/to/level1-export-m \
  -mc-closure /absolute/path/to/level1-asset-closure.json \
  -mc-asset-cache /absolute/path/to/level1-unity-asset-cache
```

It records renderer-binding evidence only. Town, combat, death/restart,
completion, window/alt-tab, and sustained GPU cases remain separate owner-lab
gates until their actual states and visual review artifacts are captured. The
optional-tier fallback route is recorded separately because it must prove the
requested tier is not silently mixed into the mandatory original cache.

The owner-bound Classic HD scenario smoke adds authoritative route observations
at the mandatory 640x480 profile:

```text
Unity -batchmode -force-glcore \
  -projectPath research/private/remc2-unity-presentation-lab \
  -executeMethod MagicCarpet.PresentationLab.Editor.CreatePresentationLabAssets.RunOwnerClassicHdScenarioSmoke \
  -mc-presentation-mode classic-hd \
  -mc-classic-profile hd-640x480 \
  -mc-package /absolute/path/to/level1-export-m \
  -mc-closure /absolute/path/to/level1-asset-closure.json \
  -mc-asset-cache /absolute/path/to/level1-unity-asset-cache
```

The receipt is
`research/private/remc2-presentation/qualification/classic-hd-owner-route-scenarios.json`.
It proves 49 persistent indexed uploads across idle, moving town, combat with
projectile/impact rows, high-speed flight, lethal player state, and tick-0
restart. This is route and Rust-observation evidence only; it does not promote
the profile to sustained GPU support or capture a visual corpus.

The optional-tier fallback receipt is
`research/private/remc2-presentation/qualification/classic-hd-owner-asset-fallback.json`.
At 960x720, requesting `optional-high-resolution` with no optional pack present
resolves explicitly to `original-indexed` and still completes the 49-frame
owner route. The native graph is not yet qualified to consume an optional pack,
so this is a fail-closed fallback proof rather than an enhanced-asset claim.

The qualification-only visual capture probe resolves the existing indexed
surface through the same palette material used by the Classic HD display. It
does not advance Rust, change the steady-state upload path, or overwrite an
existing capture. Run it with a fresh private output directory:

```text
Unity -batchmode -force-glcore \
  -projectPath research/private/remc2-unity-presentation-lab \
  -executeMethod MagicCarpet.PresentationLab.Editor.CreatePresentationLabAssets.RunOwnerClassicHdVisualCaptureProbe \
  -mc-presentation-mode classic-hd \
  -mc-classic-profile hd-640x480 \
  -mc-package /absolute/path/to/level1-export-m \
  -mc-closure /absolute/path/to/level1-asset-closure.json \
  -mc-asset-cache /absolute/path/to/level1-unity-asset-cache \
  -mc-capture-dir /tmp/magiccarpet-classic-hd-visual-captures
```

The probe emits one resolved PNG for each Level 1 Night corpus route plus
`classic-hd-owner-visual-capture-receipt.json`, including dimensions, upload
counts, frame hashes, SHA-256 file hashes, GPU metadata, and external-owner
provenance. The receipt intentionally remains `captured-for-human-review` /
`not-reviewed`; it does not convert the placeholder corpus manifests into an
acceptance claim. Review the PNGs for crushed blacks, over-bloom, fog washout,
HUD readability, exposure stability, temporal artifacts, and whether the result
still looks like the game rather than concept art before promoting a corpus.
The captured owner receipt is preserved at
`research/private/remc2-presentation/qualification/classic-hd-owner-visual-capture.json`
with its seven PNGs under
`qualification/captures/classic-hd-owner-640x480/`; the package verifier checks
the receipt hashes while keeping visual review open.

The bounded focus/pause and exit-sidecar probes are recorded in
`qualification/classic-hd-owner-focus-pause.json` and
`qualification/classic-hd-owner-exit-sidecar.json`. They prove that Unity
focus/application pause does not advance authoritative Rust state while
suspended, and that the exit presentation sidecar is clear and reset-safe at
the tick-0 boundary. Real OS window/alt-tab capture and a full authored
completion/postgame route remain open.

The runtime has a separate OS-window recorder for the remaining lifecycle
gate. It is activated only when a real player is launched with
`-mc-lifecycle-receipt`; the existing editor focus/pause probe deliberately
does not enter this trace. The recorder captures actual Unity
`OnApplicationFocus`/`OnApplicationPause` callbacks, screen mode/dimensions,
the 4:3 viewport, internal surface dimensions, allocation epoch, Rust
generation/tick, and upload count. It requires two consecutive stable rendered
frames after startup and after focus regain. It never advances Rust and it
keeps the receipt open until an external window-manager action proves the
transition.

Run a standalone, non-batch player once in windowed mode and once in
`FullScreenWindow` mode. Focus a separate real OS window, restore the player,
then exit so the recorder flushes the requested sidecar:

```text
MagicCarpetPresentationLab \
  -mc-presentation-mode classic-hd \
  -mc-classic-profile hd-640x480 \
  -mc-fullscreen false \
  -mc-window-width 1280 \
  -mc-window-height 960 \
  -mc-window-case windowed \
  -mc-desktop-session x11-or-wayland \
  -mc-window-driver external-wm-focus \
  -mc-lifecycle-receipt /tmp/magiccarpet-classic-hd-windowed-lifecycle.json \
  -mc-package /absolute/path/to/level1-export-m \
  -mc-closure /absolute/path/to/level1-asset-closure.json \
  -mc-asset-cache /absolute/path/to/level1-unity-asset-cache
```

Repeat with `-mc-fullscreen true`, `-mc-window-case fullscreen-window`, and a
separate receipt path. The checked-in
`qualification/classic-hd-owner-os-window-lifecycle.json` remains an honest
open placeholder until both external cases are captured and reviewed.

The same route is qualified separately at the 960x720 candidate profile with
`-mc-classic-profile hd-960x720`. Its receipt is
`research/private/remc2-presentation/qualification/classic-hd-owner-route-960x720-scenarios.json`.
It proves the same 49-frame route and persistent uploads at the larger internal
render size, but remains route evidence only: the measured Classic CPU/GPU
budget and sustained allocation gates still decide whether 960x720 is
promoted as a supported real-time profile.

The flight routes above are movement observations, not proof that remc2's
automatic high-speed postprocess activated. The explicit owner probe checks the
effective native mode at both candidate profiles with blur disabled:

```text
Unity -batchmode -force-glcore \
  -projectPath research/private/remc2-unity-presentation-lab \
  -executeMethod MagicCarpet.PresentationLab.Editor.CreatePresentationLabAssets.RunOwnerClassicHdHighSpeedProbe \
  -mc-presentation-mode classic-hd \
  -mc-classic-profile hd-640x480 \
  -mc-classic-blur off \
  -mc-package /absolute/path/to/level1-export-m \
  -mc-closure /absolute/path/to/level1-asset-closure.json \
  -mc-asset-cache /absolute/path/to/level1-unity-asset-cache
```

Repeat with `-mc-classic-profile hd-960x720`. The receipt is
`research/private/remc2-presentation/qualification/classic-hd-owner-high-speed-probe.json`.
The current owner ABI reaches target/actual speed 80 and remains at effective
postprocess mode 0 in both runs, so this gate stays open until a legitimate
Speed-jar activation path and GPU readback fence are qualified.

The owner effects matrix probe separately exercises the three accepted blur
tokens and proves that the owner route rejects disabled reflections or dynamic
lighting before opening the native session. Its receipt is
`research/private/remc2-presentation/qualification/classic-hd-owner-effects-matrix.json`;
this is settings-contract evidence, not a visual screenshot corpus.

Owner performance and GPU-consumption probes are separate from route evidence.
`RunOwnerClassicHdPerformanceSmoke` records an 8-frame warm-up plus 64 measured
frames, native frame time, Unity upload-submission time, allocations, hashes,
and the 33.3 ms budget. `RunOwnerClassicHdGpuFenceProbe` submits two distinct
indexed frames through the existing palette material and records raw/resolved
`AsyncGPUReadback` hashes plus graphics-fence state. The current T14
OpenGLCore result has readback pass and zero allocation growth, but the async
fence cannot report passed without async compute; Unity GPU timing and
real-time promotion remain open.

Reproduce the owner performance receipt at either internal profile:

```text
Unity -batchmode -force-glcore \
  -projectPath research/private/remc2-unity-presentation-lab \
  -executeMethod MagicCarpet.PresentationLab.Editor.CreatePresentationLabAssets.RunOwnerClassicHdPerformanceSmoke \
  -mc-presentation-mode classic-hd \
  -mc-classic-profile hd-640x480 \
  -mc-package /absolute/path/to/level1-export-m \
  -mc-closure /absolute/path/to/level1-asset-closure.json \
  -mc-asset-cache /absolute/path/to/level1-unity-asset-cache
```

Repeat with `-mc-classic-profile hd-960x720`; the measured receipt is
`research/private/remc2-presentation/qualification/classic-hd-owner-performance-smoke.json`.

The owner-bound Unity Enhanced sequence smoke is also explicit:

```text
Unity -batchmode -force-glcore \
  -projectPath research/private/remc2-unity-presentation-lab \
  -executeMethod MagicCarpet.PresentationLab.Editor.CreatePresentationLabAssets.RunOwnerUnityEnhancedSequenceSmoke \
  -mc-presentation-mode unity-enhanced \
  -mc-package /absolute/path/to/level1-export-m \
  -mc-closure /absolute/path/to/level1-asset-closure.json \
  -mc-asset-cache /absolute/path/to/level1-unity-asset-cache
```

This proves the live Rust-to-Unity presentation bridge and reset replay, not
the broader owner/GPU scenario matrix or human visual acceptance.

The owner-bound Unity Enhanced scenario smoke is explicit and covers the
currently qualified gameplay-facing presentation routes:

```text
Unity -batchmode -force-glcore \
  -projectPath research/private/remc2-unity-presentation-lab \
  -executeMethod MagicCarpet.PresentationLab.Editor.CreatePresentationLabAssets.RunOwnerUnityEnhancedScenarioSmoke \
  -mc-presentation-mode unity-enhanced \
  -mc-package /absolute/path/to/level1-export-m \
  -mc-closure /absolute/path/to/level1-asset-closure.json \
  -mc-asset-cache /absolute/path/to/level1-unity-asset-cache
```

The route is still a private owner smoke, not a visual-corpus or GPU-support
claim. The first run must be discarded if the lethal-damage boundary does not
reach the authoritative player-dead state; the receipt records only a run that
also proves projectile/impact rows and a fresh tick-0 restart.

The matching Unity Enhanced visual capture probe renders the already-applied
URP camera snapshot into a bounded 1280x720 target. It uses the same seven
Level 1 Night routes as the Classic HD probe and is likewise qualification-only:

```text
Unity -batchmode -force-glcore \
  -projectPath research/private/remc2-unity-presentation-lab \
  -executeMethod MagicCarpet.PresentationLab.Editor.CreatePresentationLabAssets.RunOwnerUnityEnhancedVisualCaptureProbe \
  -mc-presentation-mode unity-enhanced \
  -mc-package /absolute/path/to/level1-export-m \
  -mc-closure /absolute/path/to/level1-asset-closure.json \
  -mc-asset-cache /absolute/path/to/level1-unity-asset-cache \
  -mc-capture-dir /tmp/magiccarpet-unity-enhanced-visual-captures
```

It emits the seven PNGs and
`unity-enhanced-owner-visual-capture-receipt.json` with generation/tick,
active-entity/visual counts, PNG hashes, GPU metadata, and external-owner
provenance. The receipt remains `captured-for-human-review` /
`not-reviewed`; it does not promote the placeholder corpus or claim that the
whole-hog look has passed the Night quality target.
The current owner receipt is preserved at
`research/private/remc2-presentation/qualification/unity-enhanced-owner-visual-capture.json`
with its seven PNGs under
`qualification/captures/unity-enhanced-owner-1280x720/`; the package verifier
checks those hashes without turning human review into a release claim.

The owner smoke requires a temporary owner-local native plugin. Build and copy
it only for the probe, then remove the library and Unity-generated metadata
before running package verification or committing:

```text
CARGO_TARGET_DIR=/tmp/magiccarpet-gfx-spec-cargo-target cargo build --release -p sim-capi --locked
mkdir -p research/private/remc2-unity-presentation-lab/Assets/Plugins/x86_64
cp /tmp/magiccarpet-gfx-spec-cargo-target/release/libsim_capi.so \
  research/private/remc2-unity-presentation-lab/Assets/Plugins/x86_64/libsim_capi.so
# run the owner smoke, then trash libsim_capi.so and its generated .meta files
```

The lab intentionally remains plugin-free at rest; missing native input fails
the owner route closed rather than silently falling back to a synthetic world.

Verify the project boundary and URP assignment with:

```text
python3 research/private/remc2-unity-presentation-lab/qualification/verify_presentation_lab.py
python3 -m unittest discover \
  -s research/private/remc2-unity-presentation-lab/qualification/tests \
  -p 'test_*.py' -v
python3 -m unittest discover -s launcher/tests -p 'test_*.py' -v
```

The owner-free Enhanced systems have a focused Unity EditMode gate. Because
the test folder is compiled into the editor assembly, include that assembly in
the command:

```text
Unity -batchmode -nographics \
  -projectPath research/private/remc2-unity-presentation-lab \
  -runTests -testPlatform EditMode -assemblyNames Assembly-CSharp-Editor \
  -testResults /tmp/magiccarpet-enhanced-editmode.xml \
  -logFile /tmp/magiccarpet-enhanced-editmode.log
```

The current owner-free check covers terrain, actor pooling, Level 1 Night
lighting caps, water/shoreline geometry, VFX replay/reduced motion, gated
temporal upscaling, and path-free diagnostics. It is not a substitute for the
owner-bound Unity GPU/copy and preserved Exact qualification gates.
