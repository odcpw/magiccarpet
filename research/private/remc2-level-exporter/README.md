# Private remc2 Level 1 exporter

This directory contains a private preservation tool for a lawfully owned copy of
Magic Carpet 2. It initializes Level 1 through the stock remc2 program path and
exports a deterministic, pointer-free scene package. The package and all owned
game data remain inside `research/private`.

The exporter deliberately does **not** maintain a second level loader. CMake
configures the current upstream remc2 checkout as the top-level project, then
the injector clones the completed `remc2` executable target, removes only
`remc2.cpp`, and supplies `src/exporter_main.cpp` as the alternate entry point.
All engine sources, compile settings, and link dependencies therefore remain
the checkout's ones. The exporter does not edit that checkout, and its manifest
records the commit, dirty-state paths, and hashes of the actual target sources
so local preservation fixes are not mislabeled as pristine upstream code.

## Build

```sh
./scripts/build.sh
```

The build lives outside the source tree by default:
`/tmp/magiccarpet-remc2-level-exporter-build`.

## Export

```sh
/tmp/magiccarpet-remc2-level-exporter-build/remc2-level-exporter \
  --owner-root /absolute/path/to/private/MC2/runtime \
  --output /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-a
```

`--owner-root` must contain the owner-supplied `CD_Files` and `GAME`
directories used by the existing private remc2 runtime. `--output` must be a
new directory below this repository's `research/private` boundary, and its
parent directory must already exist. Validation occurs before any output
directory is created.

## Verify

```sh
python3 tests/verify_package.py \
  --source-root /home/oliver/Projects/odcpw/magiccarpet/research/private/remc2-level-exporter \
  --upstream-root /home/oliver/Projects/odcpw/magiccarpet/upstream/magic-carpet-2-hd \
  /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-a
```

For the determinism gate, export twice into two empty destinations and run:

```sh
python3 tests/verify_package.py --compare level1-a level1-b
```

The package format is `mc2-owner-level-export/v2`. Integer fields are explicit
little-endian values. Native pointers, host paths, timestamps, and memory
addresses are not package data. The verifier remains able to validate preserved
v1 packages; the exporter only emits v2.

## Package sections

- `authored-level.bin` is the exact 0x6604-byte Level 1 record, serialized
  field-by-field: header and terrain-generation parameters, 1200 authored
  features, eight wizard settings, stages, and stage variables.
- `terrain.bin` contains six row-major planes: type, height, shade, angle, and
  secondary/cave are 65536 bytes each, followed by 65536 signed 16-bit entity
  indices. Stock remc2 regression compares every plane except secondary/cave.
  On this Night map that omitted plane is inactive and its native bytes proved
  host-uninitialized across consecutive runs, so the exporter explicitly
  zeroes it. The manifest records that normalization and the preserved oracle
  hash. A Cave-map exporter must instead preserve and gate that plane.
- `entities.bin` starts `MCE1`, version, record size, and count, followed by
  1000 fixed records. Each record begins with slot, active flag, and a stable
  one-based next relation. Parameter-table and special-state pointers are
  encoded as bounded tagged relations; unknown pointers are fatal.
- `players.bin` starts `MCP1`, component version 2, record size, and count,
  followed by
  eight fixed records containing movement/controller state and complete spell
  experience, enablement, ordering, level, and selection arrays.
  `player.time_393` is an SDL/host-clock sample rather than simulation time and
  is explicitly zeroed; the deterministic global turn remains in
  `runtime.bin`. The v2 record is 573 bytes: every v1 byte remains at its
  original offset in the first 565 bytes, followed by signed little-endian
  `ambient_loop_5_distance` at offset 565 (`dword_0x19A_410`) and
  `ambient_loop_31_distance` at offset 569
  (`maxDistance_0x19E_414`).
- `runtime.bin` starts `MCR1` and contains global simulation scalars, gameplay
  settings, counters, objectives, stages, stage variables, runtime events, and
  player inputs. Every pointer-bearing union has an explicit domain tag and a
  bounded relation. The manifest discloses the one preserved-oracle scalar
  divergence: native remc2 activates Night music (`maptypeMusic = 1`), while
  the DOS regression capture had music inactive (`0`). This is classified as
  a presentation/audio selector rather than hidden as gameplay agreement.
- `presentation.bin` starts `MCV1` and contains 347 14-byte particle
  parameters, 504 level sprite selectors, and 157 34-byte entity-parameter
  records (the upstream type name retains its original-address suffix rather
  than describing its byte size).

The v2 manifest also contains `mc2-flight-reset-projection/v1`: a field-by-field
map from every Rust `FlightState` member to a package field/entity relation or
an explicit frame-zero seam default. Its expected Level 1 reset state has
entity 152 at `(19840, 56960, 5024)`, phase 153, ambient distances 2048/1746,
and `mc_sim_reset` state hash `9da4bc91af58f3ce`. The exact ordered 81-byte
state-hash encoding is versioned in the contract. The current
`NativeSim::owner_level_one` reset context has conformance hash
`77b96b82492d6f4b`.

The source-aware verifier checks headers, counts, exact record extents, entity
relation bounds, per-plane oracle hashes, the explicit inactive-plane policy,
all reset-projection mappings, the reset state hash, and both ambient values in
each preserved DOS frame 0 through 19. It also checks package file hashes and
identity, current compiled-source hashes, private permissions, and byte
identity between two independent exports.
