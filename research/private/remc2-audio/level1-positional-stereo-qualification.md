# Level 1 original positional-stereo qualification

Date: 2026-07-25  
Scope: private owner-data Track P, Level 1 IDs 37, 39, and 43 only

## Result

PASS for the bounded original command and Unity-listener contract:

- source-relative wrapped XY, listener-relative yaw, directional attenuation,
  integer pan, the 320-unit center boundary, and the 12,288-unit cutoff;
- original volume and pan wrapper quantization;
- ID 43 playback-rate variation and ID-specific sound flags;
- one pending candidate per WAV ID, the `new - old >= -8` replacement rule,
  `IfNotPlaying` duplicate suppression, and a fixed ten-voice pool shared by
  positional sounds, local feedback, and ambient loops;
- an additive 88-byte native sidecar keyed to the unchanged 72-byte event;
- exact manual stereo gains in Unity with `spatialBlend=0`, `panStereo=0`,
  Doppler 0, and no Unity rolloff; and
- seventeen post-listener windows proving left/right direction, near/far
  energy, measured rate behavior, duplicate suppression, natural
  completion/reuse, pool exhaustion/no-steal, and reset cleanup.

This does not claim the later remc2 SDL `SetSamplePositionFromPlayer` path,
vertical attenuation, environmental filtering, or exact DOS mixer resampling.

## Pinned source and recovered pipeline

The source is remc2 revision
`6806c7204d17220ecc0430759f784aea8ee2ddb2`, with `engine/Sound.cpp`
SHA-256
`2aeec6776728912747256c523b4e28a50934fa5aefa6f6009b8e85c1ee4fa6be`.
The relevant flow is:

1. `PrepareEventSound_6E450` rejects squared XY distance greater than
   `0x09000000`, equivalent to radius 12,288.
2. Wrapped 16-bit relative X/Y feed the recovered integer angle and square-root
   tables. The misleadingly named historical distance helper used by this path
   is XY-only; Z is transported for diagnostics but does not change volume or
   pan.
3. Relative yaw is in 0..1024. The directional attenuation radius is
   `12288 * (1024 - relativeYaw / 2) >> 10`: 12,288 in front, 9,216 at either
   side, and 6,144 behind.
4. The reconstructed x86 expression performs wrapping 32-bit arithmetic,
   reinterprets the volume numerator as signed, and divides with truncation
   toward zero. Values below 512 are silent and values above 32,767 clamp.
5. At distance 320 or less raw pan is 32,767. Beyond it, yaw folds around the
   rear axis and applies signed side: `32767 + foldedYaw * 64 * side`, clamped
   to 0..65,535.
6. The playback wrappers quantize volume to `raw >> 8` and pan to
   `127 * raw / 65535`. Therefore geometric center becomes pan unit 63, not 64.
7. ID 43 uses `((9377 * rng + 9439) % 30) - 15`, producing -15..+14 percent;
   IDs 37 and 39 use zero variation. The playback rate is integer
   `22050 * (100 + delta) / 100`.
8. IDs 37 and 39 retain the entity source ID as flags; ID 43 forces flags zero.
9. `ShouldUpdateSound_6EA90` replaces the one pending candidate for a WAV ID
   when `newVolume - oldVolume >= -8`. The per-frame drain maps class 3 to
   `IfNotPlaying`.
10. The mixer has ten sample handles. `IfNotPlaying` suppresses a live matching
    `(flags, WAV ID)` pair, otherwise it takes the first stopped handle. A full
    pool drops the candidate; playback completion releases the assigned handle.
    Infinite ambient loops are different: fading to volume zero does not stop
    or release them. Reactivation updates the same resident voice and continues
    sample phase; reset is the bounded release path.

The later remc2 `SetSamplePositionFromPlayer`/SDL panner is a second modern
spatializer layered after these original stereo fields. It is intentionally
excluded to avoid double attenuation and panning.

## ABI and presentation contract

`McEventV2` remains byte-for-byte 72 bytes with its reserved tail at offset 56.
`McAudioCommandV2` is a separate 88-byte record:

| Offset | Field |
|---:|---|
| 0 / 4 | ABI version / structure size |
| 8 / 16 / 24 | generation / event sequence / state hash |
| 32 | descriptor-backed asset reference |
| 36 / 40 / 44 | wrapped relative X / Y / Z |
| 48 | integer playback rate in Hz |
| 52 / 54 / 56 / 58 | sound / entity / player / flags |
| 60 / 62 / 64 / 66 | raw volume / raw pan / rate delta / voice |
| 68 / 69 / 70 / 71 | play type / signed loop count (`-1` for 1/2/5/31, `0` for effects) / action / reserved |
| 72 | four reserved u32 values |

Commands for every playable Level 1 effect are looked up by event generation
and sequence. The same native ten-voice array owns ambient loops 1/2/5/31,
local feedback 29, and positional IDs 37/39/43. Suppression actions
distinguish existing-voice, full-pool, attenuation/cutoff, and pending-candidate
replacement. Unity accepts native voice indices 0..9 and reports natural
completion back to the native session. Reset clears both sides without sending
stale completions.

Unity does not use `AudioSource.panStereo`, Unity rolloff, spatial blend, or
Doppler for this contract. It renders the mono owner sample into a temporary
stereo clip with:

`volume = volumeUnits / 127`  
`left = volume * (127 - panUnits) / 127`  
`right = volume * panUnits / 127`

The source stays at volume 1 and `panStereo=0`; playback-rate pitch is the exact
integer rate divided by 22,050.

## Deterministic and listener evidence

- `sim-core`: 100/100 tests PASS. Positional hard vectors cover eight octants,
  two rotated-yaw cases, wrapped coordinates, distance 320/321, front/side/rear
  volume thresholds, exact cutoff and cutoff+1, Z invariance, all three rate
  seeds, and pending replacement.
- `sim-capi`: 22/22 tests PASS. Layouts are checked in Rust and by compiling the
  public C header; the native vectors, signed loop count, and ten-voice
  lifecycle are exact. Loop start, `UPDATE_EXISTING`, and `EndLoop` sidecars
  all retain `loop_count=-1`.
- Owner cache: 12/12 tests PASS; 292 outputs; content SHA-256
  `041e05c8187f8900f885574aee4b356ce5e99d7998c3eab60d7e348ff1a234b6`;
  manifest SHA-256
  `3b057b4745db6c817383cfe09594669c99d8533ee16b232fa3389f25461810dd`.
- Unity Linux build: PASS.
- Headless self-test: PASS.
- Local-audio regression after shared-pool integration: 24/24 windows PASS.
- Positional/listener/live-allocator qualification: 17/17 windows PASS,
  798,720 interleaved samples, PCM16 stereo at 48 kHz, and `failures=none`.

The listener run observed a left-only hard-left vector and a right-only
hard-right vector. Near RMS exceeded far RMS.
The -15% and +14% rate windows measured 104 versus 157 left-channel zero
crossings, proving changed listener signal behavior rather than only checking
the Unity pitch property. All four negative windows contained zero nonzero
samples.

An earlier post-correction run measured 104 versus 163 crossings instead.
These post-mixer sample counts vary with Unity callback alignment; the
per-window direction, attenuation, and relative-rate assertions are the
qualification contract.

The live stages use the real session allocator and Unity completion callback:
natural completion releases voice 0 for reuse; ten simultaneous effects take
voices 0..9; the eleventh is dropped without stealing; a matching duplicate is
suppressed on voice 0; and reset while all ten voices are active increments the
generation, clears the pool, and makes voice 0 first-free again. CAPI tests also
reject completion with the wrong generation, voice, or event sequence.

Final signed-loop-count rebuilt-player temporary evidence is
`/tmp/magiccarpet-positional-loop-count-final.log` and
`/tmp/magiccarpet-positional-loop-count-final.wav`. The WAVE is observed
callback evidence, not a deterministic golden.

## Private boundary

All owner WAV data, descriptor outputs, exact source identifiers, and
qualification captures remain under the private preservation boundary or
temporary storage. The Unity project and player contain only code and the
native plugin; they load the validated owner cache externally at runtime.
Nothing in this qualification authorizes copying owner data or reconstructed
source expression into the separately authored commercial successor.
