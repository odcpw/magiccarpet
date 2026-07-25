# Level 1 audio selection proof

Scope: private, owner-data Magic Carpet 2 Level 1 preservation build only.
This is a bounded proof of the current remc2 execution path. It is not a
claim about every DOS configuration, sound card, realm, or level.

## Locked inputs

| Input | SHA-256 |
|---|---|
| `engine/LevelInit.cpp` | `8fcb4a136901adf998440d933b8cc8b3c43d78b2ab19b7cc0c1051a2dc533ca8` |
| `engine/EventsFunctions.cpp` | `fac00f7fb51f7f72996d208fb34600e8f06aee389faa3e60801be66d62fb92ac` |
| `engine/Sound.cpp` | `2aeec6776728912747256c523b4e28a50934fa5aefa6f6009b8e85c1ee4fa6be` |
| `portability/port_sdl_sound.cpp` | `dba1c0e37b6c56813e72e649aeb15f50059a36eb1be011096cacb4131630b85f` |
| `GAME/NETHERW/SOUND/MDI.INI` | `e6f5a679fd4edfd24c4c3d6b7331383cd13438f5d7d1416dc9c676652474712b` |
| qualified `runtime.bin` | `b078dbed443e2cc3bf3bfd08a0f519492546d9f939db74a3d9c185d58f9c559f` |
| owner `SOUND/SOUND.DAT` | `dba5958ae8f3aae53f3b9e3fa792bfef75a020b185e7884e344c7238ff2ee2a2` |
| owner `SOUND/MUSIC.DAT` | `3549b18c75b1812618b775e8b76f1ead312acfe4d6f5c8029d8cb696a70dbeb5` |

## Sound selection

1. `LevelInit.cpp`, `LoadLevelInitData_10650`, selects
   `LoadSounds_84300(1)` for `MapType_t::Night`.
2. The qualified Level 1 package records map type 1 (`night`) and sound bank
   1.
3. The current remc2 initialization path in `EventsFunctions.cpp` calls
   `SetSoundFreq_9A230(822)`.
4. `Sound.cpp`, `LoadSounds_84300`, maps 822 to quality descriptor index 3.
5. Parsing the pinned owner container selects:
   `bank=1`, `quality=822`, `quality_index=3`,
   `metadata_offset=1988320`, `data_offset=0`,
   `metadata_size=2080`, `data_size=1988320`.

The `hqSound=true` presentation setting controls later resampling in the SDL
adapter. It does not choose this six-row container descriptor.

ID 29 is the bounded player-local exception among one-shots:
`sub_5F660` calls `PrepareEventSound_6E450(0,
caster.playerColorIndex, 29)` for unavailable/insufficient-mana feedback.
The entity argument is zero and the player index is the caster's own color
slot. IDs 37/39/43 arise from periodic world-effect selection and use the
qualified original positional-stereo sidecar described in
`level1-positional-stereo-qualification.md`.

## Local loop orchestration

The pinned `EventsFunctions.cpp` flight-effects path makes the bounded local
loop lifecycle explicit:

- over water it prepares Ocean 1 and ends Crickets 2;
- otherwise it prepares Crickets 2 and ends Ocean 1;
- a fire-distance accumulator below 1536 prepares Fire 5, while 1536 or above
  ends it;
- a building/market-distance accumulator below 1536 prepares Market 31, while
  1536 or above ends it.

`Sound.cpp`, `PrepareEventSound_6E450`, starts these local loops at zero and
requests target volumes 70, 70, 120, and 85 respectively. `EndLoop_6EAB0`
admits local loop IDs 1, 2, 5, and 31 for this path. The Unity qualification
names its Fire/Market windows `distance_near` and `distance_far` to reflect
those already-resolved commands; positional distance arithmetic itself
remains outside this non-spatial bead.

## Music selection and preserved indexing wart

1. The pinned `MDI.INI` selects `SBPRO2.MDI`.
2. `Sound.cpp`, `InitMusic_8D970`, maps SBPRO2 to driver type `f`; container
   driver index is 2. `musicChannel_E3814` is 0.
3. Parsing the pinned owner container selects:
   `channel=0`, `driver=f`, `driver_index=2`,
   `metadata_offset=80656`, `data_offset=0`,
   `metadata_size=224`, `data_size=80656`.
4. Night sets logical music track 1. The qualified native runtime contains
   i32 value 1 at `runtime.bin` offset 32. The inactive DOS oracle captured 0;
   this remains an explicit presentation-environment divergence.
5. At game turn 1 remc2 calls `StartMusic_8E160(1,127)`.
6. Preserve the native indexing wart: the loop/AIL call site first refers to
   metadata pointer slot 1 (`C2GAME2.XMI`), while the SDL AIL wrappers subtract
   one for initialization/playback. The audible payload is therefore playback
   slot 0, `C2GAME1.XMI`. The cache contract records all three values:
   `logical_track=1`, `playback_slot=0`, `loop_scan_pointer_slot=1`.

`UpdateMusic(mode,3)` in the flight-effects journal controls the dynamic-music
fade envelope. The second argument is a timer-rate multiplier, not a group:
the timer runs at `30 * 3 = 90 Hz` and moves its integer control value by one
toward 0 for mode 1 or 127 for mode 2. It is not evidence of the initial
track-start event. That start is now modeled separately: the original loop
stops music after its first `DrawAndEvents` pass, then calls
`StartMusic_8E160(1,127)` after the second pass while local `gameTurn` is 1.
The native ABI emits one additive `StartMusic` event at the corresponding
source turn, rearms it on reset, and Unity never starts the track eagerly.
Rendered FM timbre remains a presentation adaptation rather than exact DOS
hardware output.

The two preserved music pointers expose an unresolved Level 1 distinction.
The remc2 SDL playback slot, `C2GAME1.XMI`, has no controller-119 channel tags;
it has only the controller-116/117 loop pair on channel 1. The slot-1
`C2GAME2.XMI` pointer passed to AIL contains controller-119 value-0 tags on
channels 7 and 8. Those are the channels the original trigger callback would
mark for expression-controller fades. An active SBPRO2-compatible reference
trace must decide which pointer supplies the audible sequence/channel set.
No Level 1 combat stem may be inferred from remc2's optional danger OGG.

## Qualified owner-local presentation outputs

- remc2 `TranscodeXmiToMid` converts `C2GAME1.XMI` deterministically to
  standard MIDI.
- The private cache tool renders that MIDI with the remc2-pinned libADLMIDI
  revision `6806c7204d17220ecc0430759f784aea8ee2ddb2`, embedded bank 59,
  two chips, and 44,100 Hz output.
- Two independent renders produced the identical 77,889,580-byte PCM16 stereo
  WAVE, 19,472,384 frames, SHA-256
  `03b3f951048504d8a6d30611d6c44a62ec1c463726b30c33c16e3cb69824605d`.
- Neither the renderer binary nor libADLMIDI is embedded in Unity. The WAVE
  remains in the external private cache and is streamed at runtime.

The resulting timbre is deterministic and Unity-ready, but is not claimed to
match the user's historical DOS sound-card hardware. The current stereo render
contains only remc2's playback-slot-0 payload. Unity validates and consumes
the exact source-turn start and 90 Hz dynamic-control state without incorrectly
fading the complete base track.
The exact audible tagged-channel application remains blocked on the active
reference trace described above.

## Native-to-Unity binding

When and only when `MC_ASSET_CACHE_PATH` names the exact qualified private
cache, the native loader validates the pinned cache manifest, package and
closure bindings, descriptor hashes, and every selected payload hash. It then
appends nine sound mappings plus one music mapping after the 76 frame
references. Typed audio events carry these descriptor-derived indices
76–85. Without the cache, the world remains the original 76-ref package world
and audio event references remain absent.

The preserved package reset conformance remains
`0x77b96b82492d6f4b`. Typed effects are attached only to mutable sessions and
have separately pinned initial conformance `0x00cc7b799d6917fc`; the old
package proof is not weakened or silently requalified.

## Boundary

Only proven Night/822 IDs `1,2,5,29,31,37,39,43` are materialized. ID 12 is
an explicit null mapping. Runtime playback includes local ambient loops
1/2/5/31, player-local insufficient-mana feedback 29, and original
positional-stereo IDs 37/39/43 when a complete descriptor-backed 88-byte
sidecar is present. Missing or rejected sidecars remain silent. Other Night
rows, Cave IDs 65..69, other banks, exact DOS MIDI hardware timbre, and the
optional random OGG path remain deferred.
