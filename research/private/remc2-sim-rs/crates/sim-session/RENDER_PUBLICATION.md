# Render publication v4

`sim_session::render_publication::RenderPublication` encodes a complete live
presentation observation as MCPO. The native entry point
`mc_game_runtime_render_observation_copy_v1` checks the expected runtime
generation, session generation and tick before copying one coherent packet.
Its sizing call returns the required byte count and a short-buffer status; no
packet bytes are written on that failure. A consumer must retry with adequate
space and the same guards, reacquiring current identity if the guard fails.
The packet's identity tick and observation tick refer to the same boundary.
Runtime identity, runtime generation, session identity and terrain revision must
travel together; terrain revision alone does not identify an epoch.

This is the current render projection, not an event journal. In particular,
`effect_head` is a cursor, not the retained events themselves: consumers that
need effects across skipped publications must use the retained event-journal
API and its cursor/lifetime rules. Nor is this a full campaign WASM port. The
encoder is transport-neutral Rust, and the browser reader is a reusable
`DataView` decoder; a future WASM host must provide the actual campaign runtime,
commands, assets and event consumers.

All integers are little endian and packed with no alignment. Strings are UTF-8
with a `u32` byte length, vectors have a `u32` item count, options begin with a
`u8` flag (0 absent, 1 present), and booleans are `u8` (0 or 1). Encoders
reject lengths that cannot fit in `u32`. The TypeScript and Unity readers impose
stricter **resource limits**, not additional wire-format validity: 16 MiB per
packet/string and 65,536 rows per vector, plus a check that the remaining bytes
can contain each declared row. They reject unsupported versions, truncation,
invalid UTF-8, flags or enums, and trailing bytes. TypeScript exposes `u64` as
`bigint`; the Unity DTO uses decimal strings for values beyond its JSON-facing
contract. Do not convert 64-bit counters to JavaScript `number`.

| Offset | Field |
| --- | --- |
| 0 | Four literal bytes `MCPO` |
| 4 | Version `u32` = 4 |
| 8 | Total packet byte length `u32` |
| 12 | Runtime ID `u64` |
| 20 | Runtime generation `u64` |
| 28 | Runtime mode `u8`: 0 playing, 1 paused |
| 29 | Variable-length fields below, in order |

The variable portion is:

1. Session identity: `engine_contract`, `package_graph_hash`, `definition_hash`
   strings; `definition_revision`, `session_id`, `generation`, `tick` as `u64`.
   Then terrain mutation sequence, observation tick and effect head as `u64`.
2. Player pose: `x,y` as `u16`, `z,yaw,display_pitch,effective_pitch,roll`
   as `i16`; motion `target_speed,actual_speed,strafe_speed` as `i16`; vitals
   `life,max_life,mana,max_mana` as `i32`.
3. Optional menu admission: action `u8`, four booleans `admitted,map_open,
   spells_open,player_information_open`, tick `u64`. Action values 0–8 are
   `toggle_gameplay_map`, `open_abandon_confirmation`, `confirm_abandon`,
   `cancel_abandon_confirmation`, `open_spell_menu`, `close_spell_menu`,
   `show_player_information`, `hide_player_information`, `close_gameplay_menus`.
   Follow with `return_to_title_requested` boolean.
4. Scoreboard players vector: each `source_slot u8`, `display_name` string,
   outline and text palette indices `u8`, `max_mana i32`, eight `kills8 i16`.
   Follow with optional projection-error string. Notifications vector: each
   participant `u64`, `is_local` boolean, display name and kind strings,
   `remaining_frames i16`.
5. `player_wall_contact_counter i8`, `player_immobilized_ticks u16`; camera
   `x,y u16`, `z,yaw,pitch,roll i16`, FOV `i16`; `spell_menu_has_any_spell`
   boolean, two wizard colour `u8` values, `spell_menu_panels_opaque` boolean.
6. Spells vector: each ID and display-name strings; acquired boolean; optional
   holder actor `u64`; selected boolean; tier `i8`; unlocked tier and menu slot
   `u8`; optional artwork (grid icon and three tier icons, all `u16`); mana cost
   `i32`; hand, marked hand and availability `u8`. Before mana cost, optional
   source menu help contains standard boolean, selected tier `i8`, three title
   records `u16`, three Castle-blocked booleans, mana-unavailable, enabled,
   blink-hidden and discovered booleans, three mana costs `i32`, experience
   `i32` and three experience thresholds `i32`. Hand values 0–3 are `none`,
   `left`, `right`, `both`; marked hand 0–2 are `none`, `left`, `right`;
   availability 0–5 is `ready`, `not_acquired`, `invalid_holder`,
   `invalid_caster`, `active`, `insufficient_mana`.
7. Objective: optional current-objective string, completed and total counts
   `u32`, all-satisfied boolean; textbox ticks `u8`; special announcement record
   `u16` (zero means absent); exit-active boolean.
8. Presentation entities vector: each actor `u64`, kind string, pose `x,y u16`
   and `z,yaw,pitch i16`, logical height `u16`, asset ID and content-address
   strings.
9. Two `u8` source hand-selection visibility counters, left then right, each
   in `0..=8`. End of packet immediately follows these bytes. These are an
   eight-PlayerEvents-update visibility window, not an alternating blink or
   client timer; pure reads never decrement them. A same-hand accepted source
   assignment retriggers eight. The encoder rejects out-of-range counters
   before writing any destination bytes.

Version4 extends the previous v3 packet by these final two bytes. Decoders
reject earlier versions; native and client artifacts must be installed together.

A native caller owns its destination buffer. For WASM, a JS view into linear
memory is valid only while its allocation is retained and the memory has not
grown or been overwritten. Decode synchronously or copy the bytes before either
can happen. `decodeRenderPublication` returns a deeply frozen object with no
retained reference to the input; a subsequent engine write or memory growth
cannot change that result. The protocol does not imply shared-memory atomicity:
the host must serialize copying with engine mutation or supply an equivalent
coherent snapshot.
