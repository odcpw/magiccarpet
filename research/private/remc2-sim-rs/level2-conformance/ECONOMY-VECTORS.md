# Level 2 economy conformance vectors

These vectors are independent production-path checks for bead `mc-bvihb.3`.
The reference is the pinned remc2 source object
`6806c7204d17220ecc0430759f784aea8ee2ddb2`; because the protected upstream
checkout can be dirty, every source claim below is re-derived with
`git show 6806c7204d17220ecc0430759f784aea8ee2ddb2:<path>`.

## Vectors

| Vector | Pinned source anchor | Required observable outcome | Rust witness |
|---|---|---|---|
| Death split | `remc2/engine/EventsFunctions.cpp:26867-26915`, `TransformEntityToManaSphere_36BA0` | `clamp(mana / 1000, 1, 16)` spheres; floor share first; final remainder; two sphere-local RNG steps; dying owner cleared | `level_two_economy_conformance_death_split_and_merge_vectors` |
| Sphere merge | `remc2/engine/EventsFunctions.cpp:26919-26997`, `sub_36D50` | neutral adoption; class-10 owner precedence; strict max-mana comparison with source-side tie; source disabled | `level_two_economy_conformance_death_split_and_merge_vectors` |
| Carrier AddBallon | `remc2/engine/EventsFunctions.cpp:61802-61904`, `AddBallon_60AB0` | matching owner, wrapped attraction radius, and strict AABB are all required; collection clears target and transfers all mana | `level_two_economy_conformance_addballon_carrier_vectors` |
| Castle action 4 | `remc2/engine/Events.cpp:2937-2940`; `EventsFunctions.cpp:61382-61472`, `sub_5FF50` | even-phase owned sphere collection occurs before phase advance and emits the collection event | `level_two_economy_conformance_castle_action_vectors_and_allocator_fallback` |
| Castle action 6 | `remc2/engine/Events.cpp:2954-2956`; `EventsFunctions.cpp:61261-61278`, `sub_5FCA0_destroy_castle_level` | destruction sound precedes stage-destroyed and final Castle-destroyed events; stage reaches level 0 | `level_two_economy_conformance_castle_action_vectors_and_allocator_fallback` |
| Allocator fallback | `remc2/engine/EventsFunctions.cpp:26888-26915` and `61264-61276` | exhausted arena writes dying-source RNG/owner state but creates no sphere/event; action-6 no-free branch returns action 4 without teardown | `level_two_economy_conformance_castle_action_vectors_and_allocator_fallback` |
| Production anchor | `remc2/engine/EventsFunctions.cpp:61162-61216`, `BeginOfCastleCreation_5FA70`; `28071-28108`, `RemoveCastleStage_385C0` | source-grid snapped root and construction anchor remain distinct inputs; model-42 child uses the authored anchor | `level_two_economy_conformance_castle_action_vectors_and_allocator_fallback` |

The vectors intentionally do not claim exact `sub_5F810` defender target
arbitration; that remains an explicit Level 2 gap in the Castle slice.

Reset/negative evidence is part of the witnesses: the split test pins the
hard-coded `999..=115` free stack and reset hash, while the carrier test covers
rejected owner/distance/AABB inputs and the Castle test covers both no-free
allocator branches with no spurious events.
