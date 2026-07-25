# Conformance coverage

This matrix describes the private source-derived milestone. `Partial` means
that a behavior is present and tested without claiming exact-build parity.

| Surface | Atlas/specification coverage | Tests | Status |
|---|---:|---:|---|
| Embedded semantic atlas | 24/24 tables; 14,496/14,496 keyed rows | 4 | Passing |
| Command layout and latching | Recorded 10-byte layout, both same-signature recording variants, real raw-command-to-decoded-state proof, one-latch/many-pass cadence, both full-stop forms, opcode 6 selected-spell resolution before fire | 7 | Partial |
| Outer pass scheduler | Exact source mapping for speed indices 0/1/2 to 1/4/8 passes and all other bytes to 0; focused state checks plus pinned-native level-1 state checks after every pass in 4/8 modes | 3 | Partial |
| Forward, strafe, yaw, pitch, recoil, impulses | Mapped deterministic spine, both strafe signs/clamps, signed yaw truncation, recoil cap/order, exact target-speed-change marker, source-generated movement constants | 12 | Partial |
| Terrain-relative pitch | Sign matrix, `1024` boundary, altitude clamp, zero-input retention, speed-before-pitch order | 3 | Partial |
| Speed modifier and immobilization | Control/translation shaping, stage counters, palette/move-speed flags, movement stop, vertical immobilization drift | 3 | Partial |
| Material redirection and recovery | Exact `0x100` identity, low-word 3D distance truncation, first- and second-direction boundary cases, outdoor rejection order, counter decay/refresh, exact recovery latch and push | 8 | Partial |
| Cave collision | Strict overlap boundary, material failure, equal-gap acceptance, rejection, target stop, side steering, mutually exclusive ground/ceiling repair, recovery conditions | 8 | Partial |
| Active tether | Typed target/holder projection, positive/negative spring-speed caps, turn cap, tier effects, exact range boundary, dead target, invalidation, and an owned level-1 native target/holder checkpoint | 6 | Partial |
| Collision-time speed spell mutation | Typed holder projection and cave rejection reset | 1 | Partial |
| Concrete original terrain provider | Exact ground/cave interpolation, material switch, flags, edge wrapping | 4 | Partial |
| Isolated Level 1 indexed terrain renderer | Exact DrawSky seed, live 40x21 lattice/projection, all UV rows, 256-stride BL32N addressing, reflection-before-primary scheduling, source mode 5 and framebuffer-dependent mode 26, reset and moving-water hashes, near/fog/far census, both diagonals, all D4 groups, toroidal seam pair, and generation/tick-checked 64,000-byte index/coverage ABI | 59 focused camera/renderer tests plus integrated owner vectors | Passing within explicit sky+terrain-only boundary |
| Exact-build state/terrain/recording decoding | Original 224,790-byte state frames, `0x70000` terrain frames, both recording formats, player/entity/transient pointer consistency | 6 | Passing |
| Exact-build flight transitions | 475 idle transitions over all 25 levels (including 8 cave maps), 19 mid-game idle transitions, 24 consecutive raw-command-to-state outdoor transitions ending in 5 acceleration passes, and pinned-native per-pass forward/steering/reverse/strafe/pitch/recoil/modifier/immobilization plus real-map material/cave reject/recovery/tether traces | 11 | Partial |
| Pinned native reference runner | Standalone two-tick full-state comparison passes; all 23 enabled level-start regressions pass; isolated after-load idle regression passes; action-runner failure is localized to the current loader consuming metadata from a legacy bare recording | diagnostic gate | Passing with documented harness defect |
| Deterministic flight effects | Complete periodic-sound switch, cave RNG sound/no-sound, outdoor water/wind loops, near/far distance loops, local/non-local music branches, early-return bypass | 4 | Partial |
| Spell-fire dispatch boundary | Left/right/selected holder projections, exact request order, direction masks `256/512/256`, synchronous execution with holder aliasing, dispatch before one-pass flight skip | 2 | Partial |
| Spell activation state machine | Sentinel, every holder-model branch family, signed gates/timers, mana threshold, caster special case, direction/pointer bits, help/sound event order, and an owned level-1 native left-fire checkpoint | 10 | Partial |

Current atlas overlay: 43 `Partial`, 359 `StructuralOnly`, and 14,094
`MappedOnly`. The workspace currently runs 197 tests: 159 `sim-core`, four
`atlas-registry`, 30 `sim-capi`, and four parked `sim-wasm` tests. No row is
labeled `Implemented` or `Verified`.
