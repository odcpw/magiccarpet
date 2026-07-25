# Known Level 1 Unity audio conformance divergences

Date reviewed: 2026-07-25  
Scope: private owner-data Track P, Level 1 only

## DISC-001: Original positional stereo command fields are absent — RESOLVED

- **Reference:** remc2 derives source-relative position, integer attenuation,
  yaw-relative pan, playback rate, and arbitration before sample playback.
- **Current Unity path:** **RESOLVED** by `mc-dl0r.17.11.6.9`. An additive
  88-byte sidecar transports exact integer position, volume, pan, rate,
  arbitration action, and native voice ownership while `McEventV2` remains
  unchanged.
- **Impact:** bounded parity is now claimed for Level 1 IDs 37/39/43 only.
- **Resolution evidence:** `level1-positional-stereo-qualification.md`;
  100/100 sim-core tests, 22/22 sim-capi tests, 24/24 local-audio windows,
  and 17/17 positional/live-allocator Unity listener windows PASS.
- **Remaining boundary:** Unity intentionally keeps `spatialBlend=0`,
  `panStereo=0`, Doppler 0, and no rolloff. Vertical attenuation and remc2's
  later SDL positional layer are not claimed.

## DISC-002: Dynamic-music pointer/channel application is unresolved

- **Reference:** `UpdateMusic(mode, 3)` means target mode plus a fade-rate
  multiplier of 3, producing a 90 Hz integer envelope. The second argument is
  not a group ID. `C2GAME1` has no controller-119 tags, while the slot-1
  `C2GAME2` AIL pointer tags channels 7 and 8.
- **Current Unity path:** the exact 0..127, one-unit-per-callback envelope,
  90 Hz timer, endpoint retirement, target reversal, start/reset guards, and
  event values are implemented and qualified. The deterministic
  playback-slot-0 stereo render remains unchanged because its tagged-channel
  set is empty. The complete base track is never faded as a substitute.
- **Impact:** the controller state and its Level 1 audible no-op are preserved.
  Whether an active original SBPRO2 path instead applies the slot-1 channel
  tags remains unresolved.
- **Resolution:** **PARTIALLY RESOLVED** by `mc-dl0r.17.11.6.11.2`; active
  pointer/channel tracing remains in `mc-dl0r.17.11.6.11.3`.
- **Tests affected:** Rust pins all envelope transitions; Unity headless pins
  start, ramp, retirement, reversal, and reset. Optional danger OGG is
  excluded.

## DISC-003: Start timing is resolved; historical hardware timbre is bounded

- **Reference:** remc2 stops music after the first `DrawAndEvents` pass and
  calls `StartMusic(1,127)` after the second while local `gameTurn` is 1.
- **Current Unity path:** the native session emits one descriptor-backed
  `StartMusic` event at the matching source turn; reset rearms it and Unity
  does not play before that event. C2GAME1 is rendered deterministically with
  pinned libADLMIDI bank 59.
- **Impact:** start order and one-start-per-generation behavior are qualified.
  Historical hardware timbre is still not claimed.
- **Resolution:** start lifecycle **RESOLVED** by
  `mc-dl0r.17.11.6.11.2`; timbre remains explicitly bounded in
  `mc-dl0r.17.11.6.11.3`.
- **Tests affected:** Rust C ABI tests cover reset and two-session independence;
  Unity headless and graphical autoplay consume the typed start event. The
  pre-mixer WAVE hash is deterministic.

## DISC-004: Fire/Market qualification consumes resolved near/far commands

- **Reference:** the flight-effects path uses 1536-unit accumulators to choose
  `PrepareSound` or `EndLoop`.
- **Current Unity path:** the local-audio qualifier proves both resolved
  command outcomes and listener fades, but does not independently recompute
  the distance accumulator.
- **Impact:** this bead proves playback lifecycle, not spatial/distance
  authority.
- **Resolution:** **ACCEPTED** for `mc-dl0r.17.11.6.10`; command-production and
  positional arithmetic remain separate conformance work.
- **Tests affected:** `fire_distance_*` and `market_distance_*` windows.
