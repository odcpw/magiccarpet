# Level 1 Unity owner-audio qualification

Date: 2026-07-25  
Scope: private owner-data Track P, Level 1 only

> Historical audio milestone record. The playable Level 1 build has since
> expanded the owner cache to 379 outputs and twenty descriptor-backed audio
> references (native asset-ref range 76–95). Current artifact pins and the
> final cross-system verdict are in
> `../remc2-unity-native/LEVEL1-READINESS.md`; the observations below retain
> the evidence from this earlier bounded audio qualification.

## Result

PASS for the bounded Level 1 presentation contract:

- exact qualified Night bank 1 / quality 822 source records;
- descriptor-backed native event references 76–85 when the exact cache is
  attached;
- deterministic per-session typed effects journal;
- Unity clip decoding, loop command envelope, streaming base music, and
  post-listener non-silent PCM;
- 24 bounded post-listener local-audio windows covering audible, continuous,
  fading, silent-negative, and real native-reset outcomes;
- no owner audio embedded in Unity `Assets` or `Builds`.

This local-loop qualification is not itself a claim of exact DOS hardware
timbre or the separable war-music layer. Positional IDs are qualified
separately in `level1-positional-stereo-qualification.md`.

## Deterministic cache

- Outputs: 293
- Cache content SHA-256:
  `9a9aee892f2fe2c9fe98c98680f7f537c80bd5e531b41a91f146d0cae08910a4`
- Manifest file SHA-256:
  `77f90fcdea2ab7e0cb262d4fc0b06de02e12a01c9c49fbd4537441610bcd19a9`
- Cache tests: 13/13 PASS in 77.313 seconds, including two byte-identical
  rebuilds, Goat-sample tamper/truncation negatives, and the exact
  playback-slot/loop-scan controller census.
- Deterministic music WAVE: 77,889,580 bytes, PCM16 stereo, 44,100 Hz,
  19,472,384 frames, SHA-256
  `03b3f951048504d8a6d30611d6c44a62ec1c463726b30c33c16e3cb69824605d`.

## Native and managed gates

- `cargo test -p sim-core native_world --locked`: 9/9 PASS.
- `cargo test -p sim-capi native_v2 --locked`: 14/14 PASS.
- Preserved package reset conformance:
  `0x77b96b82492d6f4b`.
- Additive Level 1 session/effects conformance:
  `0x00cc7b799d6917fc`.
- Music-lifecycle Unity Linux player build: PASS, 86,129,922 bytes reported by
  Unity; log `/tmp/magiccarpet-unity-build-music-qualified-final.log`.
- Local-audio qualification Unity Linux player build: PASS, 85,893,830 bytes
  reported by Unity.
- Final headless self-test: PASS with
  `ownerAudio=Night822/C2GAME1/refs10/shared10voices/positionalStereo`.
- Music lifecycle self-test: PASS for source-turn start, initial
  0/idle/-1 control state, 127 upward callbacks, following-callback
  retirement, mode-1 reversal from 127, and reset silence/rearm; log
  `/tmp/magiccarpet-native-selftest-music-qualified-final.log`. The same test
  forces a failure after two effect rows and proves the partially constructed
  music clip, effect clips, sources, root, and stream owner take the
  transactional disposal path.
- Graphical autoplay: PASS at tick 443 with exactly one
  `MC_MUSIC_START logicalTrack=1 playbackSlot=0 loopScanPointerSlot=1
  sourceGameTurn=1`; log
  `/tmp/magiccarpet-autoplay-music-qualified-final.log`.
- The envelope self-test proves start at zero, ten 60 Hz callbacks reach
  integer volume 10, repeated `PrepareSound` continues to 15 without restart,
  and `EndLoop` returns 15 to zero in fifteen callbacks.

## Bounded local-audio listener qualification

The dedicated `-mc-audio-qualify-local` path disables base music and simulation
stepping, then drives typed `PrepareSound`/`EndLoop` events through the normal
owner-audio presentation. Its timeline advances from the number of samples
actually received by the sole Unity listener tap. This avoids falsely
advancing stages when a render loop starts before the audio device callbacks.

The 24-window matrix covers:

| Surface | Listener evidence and state invariant |
|---|---|
| Initial state | 0 nonzero samples in bounded pre-roll |
| Ocean | audible early/late ramp; late RMS exceeds early RMS; exact target 70 |
| Ocean → crickets | no silent 20 ms block; ocean reaches 0 but remains resident; crickets reaches 70 |
| Crickets silent residency | audible fade followed by a zero-sample window while the same voice remains playing, advances sample phase, and reactivates without restart |
| Fire near/far command result | audible ramp/target 120/fade, then exact silence with its voice resident |
| Market near/far command result | target 85; repeated prepare advances time without another source start or silent block; fade to a silent resident voice |
| CANTUSE | exactly one local-feedback play; audible window followed by silence |
| NULL | no source/count change and a silent window |
| Positional ID 37 without its required sidecar | `MC_AUDIO_UNSUPPORTED` plus a silent fail-closed window |
| Unknown ID 999 | `MC_AUDIO_UNKNOWN` plus a silent window |
| Reset/generation | audible generation-1 loop; real native reset reports generation 2; no loops, one-shots, music, or listener samples survive |

“Near” and “far” name the already proven upstream command outcomes:
`PrepareSound` versus `EndLoop`. The separate positional qualifier now covers
the original distance/pan/rate arithmetic for IDs 37/39/43.

Final observed run:

- log: `/tmp/magiccarpet-local-audio-qual-generation.log`;
- capture: PCM16 stereo, 48,000 Hz, 18.026667 seconds;
- 1,730,560 interleaved samples, 1,271,430 nonzero;
- peak amplitude 0.668132;
- 24/24 `MC_AUDIO_WINDOW ... pass=True`;
- `MC_AUDIO_LOCAL_QUAL PASS ... failures=none`;
- temporary private capture SHA-256:
  `8f1725b319fb3d18ebbaed05fa3ad26c283ab699665f3d9be4d14bbffb8de026`.

An independent second run also passed 24/24 windows and the native generation
1 → 2 reset check. It produced 1,708,032 samples, 1,242,834 nonzero,
17.792000 seconds, the same 0.668132 peak, and a different observed SHA-256:
`b63493b7053287a15b7f15fbe0413d830291db44729a49b578c8e448bc7781ee`.

The temporary WAVE hash is not a golden. Callback alignment varies; the
per-window audible/silent thresholds, continuity checks, typed command log,
exact loop-state checks, and native generation transition form the verdict.

The final signed-loop-count rebuilt-player shared-pool run at
`/tmp/magiccarpet-local-loop-count-final.log` passed all 24 windows with
1,738,752 interleaved samples and `failures=none`. Its zero-volume crickets
window contained no nonzero samples while the same voice stayed playing,
advanced its monotonic sample-phase counter, accepted `UPDATE_EXISTING`
without another play start, and remained allocated until the reset stage.
The typed command validator also requires signed `loop_count=-1` for descriptor
loop rows and `0` for non-loop effects.

A missing-capture negative run exited 2 from synchronous `MC_FATAL`, emitted
no local-audio PASS, and created no capture.

## Actual Unity mix capture

Two initial Pulse null-sink monitor recordings were silent even though the
FMOD sink input was routed to the transient sink. This was retained as a
failed diagnostic, not promoted as a pass.

The runtime probes then showed:

- `AudioListener.volume=1`, `AudioListener.pause=false`;
- source playing and `timeSamples` advancing;
- at tick 200: 108 stream reads and 419,253 nonzero callback samples.

A qualification-only `OnAudioFilterRead` tap on the sole AudioListener then
captured the post-listener Unity mix during a successful 443-tick autoplay:

- PCM16 stereo, 48,000 Hz, 9.600 seconds;
- 921,600 interleaved samples;
- 868,327 nonzero samples;
- peak amplitude 0.157493;
- FFmpeg mean volume -30.9 dB, maximum -16.1 dB;
- first observed temporary capture SHA-256
  `cb64476cce28c7a99b19b916e8b0ca406303bb1bba0cfd6a525b681349dc7299`.

A second reviewed capture had the same format, duration, peak and volume
metrics, but 870,375 nonzero samples and SHA-256
`f76584994984bf0999f5dc21ae6651b88a844fbc2045995f9372dad6b337642f`.
These post-mixer hashes are observed evidence, not repeatable goldens; Unity
audio callback scheduling changes sample alignment. The deterministic
pre-mixer music WAVE hash remains the golden.

The final suppression/confinement build was captured once more during a
successful 440-tick autoplay: 9.557333 seconds, 917,504 samples, 864,231
nonzero, peak 0.157493, mean -30.9 dB, maximum -16.1 dB, observed SHA-256
`f68c59fffd38549728948c8dbb039d412333fe781fe09e0938fa9d6f36fbb61b`.

The source-turn music build produced a fresh observed listener capture during
the successful 443-tick run: PCM16 stereo at 48,000 Hz, 10.474667 seconds,
1,005,568 interleaved samples, 915,431 nonzero, peak 0.122979, mean -32.6 dB,
maximum -18.2 dB, observed SHA-256
`26a2ef0d4f754a7c80f62b6d0b4edbd56cdf5c68bf46a19f3245262135763b21`.
This ordinary autoplay capture has no staged windows, so its logged
`windowPass=False` means “no window matrix requested,” not a failed capture.
Its callback-aligned hash is observed evidence rather than a golden.

The tap is dormant unless `MC_AUDIO_CAPTURE_PATH` is explicitly set. The path
must be a direct descendant of the system temporary root with an existing,
non-symlinked parent chain; Unity project/build paths and parent escapes are
rejected before file creation. The
transient `mc2_qual` null sink was unloaded and the original speaker sink was
restored after capture.

A negative symlink-path run synchronously failed in
`MagicCarpetRuntime.Awake` with `MC_FATAL`, exited 2, emitted no autoplay PASS,
created no output under Unity Assets, and removed its transient symlink.

## Explicit deferrals

The numbered discrepancy register is
`research/private/remc2-audio/level1-unity-audio-discrepancies.md`.

- The additive positional sidecar now transports proven position, pan,
  per-event volume, playback rate, action, and voice index for IDs 37/39/43.
  Unity spatial blend, `panStereo`, Doppler, and default rolloff remain
  disabled because exact manual stereo gains are applied to the owner PCM.
  A positional event without a complete sidecar is still silent and emits a
  distinct structured `MC_AUDIO_UNSUPPORTED` record. ID 29 remains playable
  as player-local feedback.
- `UpdateMusic(mode, 3)` controls a 90 Hz integer dynamic-music envelope; `3`
  is the timer multiplier, not a group. Playback slot 0 has no controller-119
  channel tags while AIL pointer slot 1 tags channels 7 and 8, so the exact
  envelope state is advanced without fading the complete stereo track. Music
  starts only from the qualified source-turn event and reset rearms that gate.
  Audible tagged-channel application is deferred pending an active reference
  trace.
- Unknown mappings are silent, emit a structured `MC_AUDIO_UNKNOWN` record,
  and fail qualification.
- The music render is a deterministic owner-local libADLMIDI presentation
  adaptation, not exact DOS sound-card output.
- Exactness is bounded to Level 1 target modes 1/2 and integer callback state.
  Generic invalid-mode behavior, audio-device wall-clock phase, historical DOS
  hardware timbre, and the unresolved slot-1 tagged-channel route are not
  claimed.
