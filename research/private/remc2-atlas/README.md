# remc2 exact-build private gameplay candidate matrix

Status: active Track P archaeology  
Tracking beads: `mc-dl0r.13`, `mc-dl0r.14`, `mc-dl0r.15`, `mc-dl0r.16`  
Classification: `PRIVATE_ANALYSIS`

This directory contains a reproducible, source-backed inventory of the pinned
Magic Carpet 2 HD/remc2 reconstruction. It is not a claim that static source
analysis alone proves the complete semantics of the original DOS executable.

The map is exhaustive only against declared denominators:

- the exact nested Git commit and dirty-diff hash in `manifest.json`;
- the translation units selected by the remc2 CMake target;
- the declared compiler, target, build configuration, and preprocessor flags;
- the source and loaded-data surfaces explicitly listed in the coverage
  ledger; and
- every unresolved parse, alias, indirect-dispatch, table-lineage, or runtime
  gap recorded rather than silently discarded.

## Evidence stages

Candidate rows progress independently through:

1. `DISCOVERED`
2. `STATIC_CONNECTED`
3. `RUNTIME_READ`
4. `CAUSALLY_VALIDATED`
5. `CORROBORATED`

Static output begins at `DISCOVERED` or `STATIC_CONNECTED`. It never promotes a
literal or field to a gameplay parameter merely because it has a suggestive
name or comment.

## Semantic classes

The later classification pass assigns one of:

- `GAMEPLAY_PARAMETER`
- `INITIAL_CONDITION_OR_CONTENT`
- `DERIVED_VALUE`
- `RUNTIME_STATE`
- `STRUCTURAL_CONSTANT`
- `ID_SELECTOR_MASK_OR_SENTINEL`
- `CONTEXT_DEPENDENT_STORAGE_VIEW`
- `CONTEXT_DEPENDENT_RUNTIME_SLOT`
- `PRESENTATION`
- `PLATFORM_OR_COMPATIBILITY`
- `DEBUG_OR_TEST`
- `UNKNOWN`

Loaded values require a lineage such as:

```text
compiled seed
  -> file/config/level/save load
  -> mode or level patch
  -> derived value
  -> authoritative or presentation consumer
```

## Generated artifacts

`build_atlas.py` writes an owner-only SQLite database and derived summaries:

- `atlas.sqlite`: authoritative sparse inventory and edge store;
- `manifest.json`: source/build/parser provenance;
- `coverage.json`: explicit numerators, denominators, and gaps;
- `summary.json`: counts suitable for human review; and
- `subsystem-seeds.csv`: conservative filename-based starting clusters, never
  semantic proof.

The database contains translation units, functions, parameters, records,
fields, globals/statics, macros, literals, includes, direct and unresolved
calls, field/global accesses, recognized bulk-memory operations, and parser
diagnostics.

`render_atlas.py` converts the database into reviewable sparse matrices:

- `function-matrix.csv`;
- `call-edges.csv`;
- `state-access-edges.csv`;
- `state-targets.csv`;
- `subsystem-call-matrix.csv`;
- `subsystem-state-matrix.csv`;
- `subsystem-call-graph.dot` / `.svg`; and
- `atlas-overview.md`.

`gameplay-system-map.md` is the human architecture map over those mechanical
outputs.

`build_semantic_backbone.py` then joins high-value code paths to the
hash-verified owned spell table, localization maintenance data, and
day/night/cave icon packs. `build_semantic_coverage.py` links those claims back
to the complete 3,060-function and 3,246-state-target denominators, emits
ranked raw-gap queues, and records the architectural and per-tick spines.
`build_dispatch_atlas.py` separately closes the complete indirect-table
denominator: 845 action/constructor rows, including disabled rows, sentinels,
intentional no-call cases, and missing reconstructed cases. Owner-only outputs
under `semantics/generated` currently cover 26 families, 78 tiers, 390 spell
icon bindings, 1,572 full HSPR/MSPR sprite bindings, 337 semantic claims,
all 16 runtime class families, all 29 class-5 model slots, 250
constructor-slot annotations, all 546 action rows classified, and an explicit
unresolved ledger. See
`semantics/README.md` and `semantics/whole-program-semantic-map.md`.

## Running

The preferred exact run uses the compilation database produced by the pinned
CMake build:

```bash
cmake \
  -S upstream/magic-carpet-2-hd \
  -B /tmp/remc2-atlas-cmake \
  -DCMAKE_BUILD_TYPE=Debug \
  -DCMAKE_EXPORT_COMPILE_COMMANDS=ON

python3 research/private/remc2-atlas/build_atlas.py \
  --source-root upstream/magic-carpet-2-hd \
  --compile-commands /tmp/remc2-atlas-cmake/compile_commands.json

python3 research/private/remc2-atlas/render_atlas.py

research/private/remc2-atlas/build_semantic_backbone.py

python3 research/private/remc2-atlas/build_dispatch_atlas.py

python3 research/private/remc2-atlas/build_semantic_coverage.py

dot -Tsvg \
  research/private/remc2-atlas/generated/subsystem-call-graph.dot \
  -o research/private/remc2-atlas/generated/subsystem-call-graph.svg
```

### Private owner-data asset catalogue

`build_owner_asset_catalog.py` inventories a configurable owned installation
without embedding or copying any source asset bytes. It emits:

- `semantics/generated/owner-asset-catalog.json`, the complete deterministic
  file and entry catalogue;
- `owner-asset-catalog-summary.json`, the compact counts and catalogue hash;
  and
- `owner-asset-catalog-unresolved.csv`, every opaque, malformed, unbound, or
  not-yet-decoded entry.

The catalogue preserves source SHA-256 addresses, RNC container metadata,
LEVELS and TMAPS offsets, bitmap dimensions, palette bindings, and separate
hashes for decoded palette indices and visibility masks. A PNG preview is
never treated as the indexed source identity. Optional remc2 enhanced assets
have a separate provenance and require their own release review.

```bash
python3 research/private/remc2-atlas/build_owner_asset_catalog.py \
  --owner-root research/private/remc2-native-runtime/CD_Files \
  --enhanced-root upstream/magic-carpet-2-hd/enhancedassets

python3 -m unittest discover \
  -s research/private/remc2-atlas/tests \
  -v
```

Use `--no-enhanced` for an owner-data-only catalogue. Paths inside the
generated manifest are root-relative, so moving an identical owned
installation does not change its catalogue hash. Realm identifiers follow the
runtime contract exactly: `0 = day`, `1 = night`, and `2 = cave`; the Level 1
regression fixture records `MapType = 1` and therefore uses the night banks.

A conservative fallback run is available when the exact compilation database
is unavailable:

```bash
python3 research/private/remc2-atlas/build_atlas.py
```

Fallback results remain `PROVISIONAL` and every translation unit with an error
diagnostic remains an open coverage gap. No parser failure is treated as a
successful map.

The native CMake configure uses the normal Arch packages `sdl2_image`,
`sdl2_mixer`, `sdl2_ttf`, `spdlog`, `nlohmann-json`, and `wxwidgets-gtk3`.

## Static-analysis limits

The first extractor is deliberately conservative:

- packed structures and unions may alias several field names;
- byte buffers, casts, pointer arithmetic, serialization, and bulk memory
  operations can bypass typed field references;
- address escapes and non-const pointer arguments are possible writes;
- callbacks, function pointers, event/type/subtype dispatch, and jump tables
  may remain unresolved;
- compile-time platform/configuration branches require separate manifests;
- a loaded table may overwrite a plausible compiled initializer; and
- unobserved code is `UNOBSERVED`, not automatically dead.

These gaps are inputs to the later runtime and DOSBox/native correlation work.
Raw source-derived identifiers, offsets, addresses, tables, and graphs remain
inside this private lab.
