# Private remc2 owner-level exporter

This directory contains a private preservation tool for a lawfully owned copy of
Magic Carpet 2. It initializes an owner-data level through the stock remc2
program path and exports a deterministic, pointer-free scene package. The
external level number is one-based and defaults to Level 1; remc2 receives the
corresponding zero-based internal index exactly once. The package and all owned
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

The build refuses to compile the protected `upstream/magic-carpet-2-hd`
checkout when it is dirty and accepts only remc2 commit
`6806c7204d17220ecc0430759f784aea8ee2ddb2`. Materialize that committed source
in an isolated clean Git checkout and point the build at it:

```sh
MC2_LEVEL_EXPORTER_SOURCE_ROOT=/absolute/path/to/clean/remc2 \
  ./scripts/build.sh
```

The build lives outside the source tree by default:
`/tmp/magiccarpet-remc2-level-exporter-build`.

## Export

```sh
/tmp/magiccarpet-remc2-level-exporter-build/remc2-level-exporter \
  --owner-root /absolute/path/to/private/MC2/runtime \
  --output /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-a \
  --level 1
```

`--level N` accepts external levels 1 through 25 and is optional (`1` is the
default). Invalid, missing, duplicate, signed, or out-of-range values are
rejected before the output directory or its `.staging` sibling is created.
External Level 2 selects remc2 internal index 1 and must resolve to the Day
profile; its manifest records both values. The Level 1 path keeps the sealed
Night profile and its exact source-derived assertions.

`--owner-root` must contain the owner-supplied `CD_Files` and `GAME`
directories used by the existing private remc2 runtime. `--output` must be a
new directory below this repository's `research/private` boundary, and its
parent directory must already exist. Validation occurs before any output
directory is created.

## Verify

The strict verifier is sealed to the preserved historical Level 1 v3 package
identity. Run it against that preserved package with the matching pinned source
checkout; do not substitute the protected dirty worktree or a newly built
clean-source package, whose provenance is intentionally different. The native
clean-source profile and determinism gate below are the acceptance checks for
new exports. The generic current-source verifier is introduced by the next
conformance slice.

```sh
python3 tests/verify_package.py \
  --source-root /path/to/magiccarpet/research/private/remc2-level-exporter \
  --upstream-root /path/to/matching/pinned-level1-source \
  /path/to/magiccarpet/research/private/generated/plan24-p-final3-20260730
```

For the determinism gate, export twice into two empty destinations and run:

```sh
python3 tests/verify_package.py --compare level1-a level1-b
```

The package format emitted by this exporter is `mc2-owner-level-export/v3`.
Integer fields are explicit
little-endian values. Native pointers, host paths, timestamps, and memory
addresses are not package data. The verifier remains able to validate preserved
v1 and v2 packages remain verifiable; the exporter emits v3.

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
  `runtime.bin`. The player-record v2 layout is 573 bytes: every v1 byte remains at its
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

The v3 manifest also contains `mc2-flight-reset-projection/v1`: a field-by-field
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

Focused selector tests use the production header and make the native CLI gate
explicit. This is a touch-triggered auxiliary lane, separate from the default
remc2/Rust/Unity development lane. Pytest is the canonical runner: it discovers
the existing tests and exits nonzero when zero tests are selected.

```sh
python3 -m pytest tests -v
MC2_LEVEL_EXPORTER_BIN=/absolute/path/to/remc2-level-exporter \
MC2_LEVEL_EXPORTER_RUN_FULL=1 \
  python3 -m pytest tests -v
```

The first command always runs the external-to-internal mapping and rejection
tests; it prints a build-gated skip for native CLI checks when no binary is
provided. The second command runs invalid-argument no-partial-output checks
and four full production captures (default Level 1, explicit Level 1, and two
Level 2 captures), comparing complete trees and asserting the Level 2 manifest
is external `2`, internal `1`, and Day. Build the binary only from the clean
pinned source described above.

## Remaining-realm campaign census

The bounded census tool is `scripts/campaign_census.py`. It is the production
route for the private `mc-iol7n` evidence receipt: it invokes this pinned
exporter exactly twice for each external Level 3 through 24, compares every
regular file and manifest identity, and records only hashes plus bounded
authored/reset tuples, objective sections, map gates, owner source facts, and
capability diffs. It never launches the original executable or changes the
Rust/Unity implementation.

Run it from the repository root after the clean exporter build and with the
existing private owner runtime, Level 2 evidence baseline, and source dispatch
atlas:

```sh
python3 research/private/remc2-level-exporter/scripts/campaign_census.py
```

The exact ignored output roots are
`research/private/generated/mc-iol7n-campaign-census-run-a/` and
`research/private/generated/mc-iol7n-campaign-census-run-b/`. The committed
receipt and bounded Markdown summary are
`census/campaign-census-mc-iol7n.json` and
`census/campaign-census-mc-iol7n.md`.

The tool fails closed if a realm cannot be represented by the current exporter
profile. In particular, the current non-Level-1 v5 path requires a nonempty
authored class-10/model-45 collision-mask selector set; a qualified realm with
none needs an explicitly reviewed empty-mask or alternate package contract.
The census must not invent a selector or skip that realm.
