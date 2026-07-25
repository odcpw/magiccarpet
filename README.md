# Magic Carpet private preservation lab

This repository is the private, owner-controlled research and preservation
workspace for understanding *Magic Carpet 2* and developing a faithful
Rust-driven Unity flight viewer. It is not the publication repository for the
original commercial successor.

The repository intentionally tracks authored source, tests, policy, and
research documentation while excluding:

- owned GOG installers, manuals, game data, saves, captures, and audio;
- generated level packages, decoded owner-asset caches, analyzer projects, and
  raw runtime evidence;
- Unity `Library` and player builds, Rust `target`, Node dependencies, and
  other reproducible build products;
- Beads databases, agent state, local logs, and unreviewed transfer staging.

Read these documents before changing the project:

1. [`AGENTS.md`](AGENTS.md)
2. [`docs/program-charter.md`](docs/program-charter.md)
3. [`docs/product-promise.md`](docs/product-promise.md)
4. [`docs/preservation-track.md`](docs/preservation-track.md)
5. [`docs/stack-decision.md`](docs/stack-decision.md)
6. [`docs/provenance-policy.md`](docs/provenance-policy.md)
7. [`docs/private-public-boundary.md`](docs/private-public-boundary.md)

## Active implementation

- `research/private/remc2-sim-rs/` — deterministic renderer-free Rust
  simulation and native ABI.
- `research/private/remc2-unity-native/` — Unity desktop presentation and
  qualification harness.
- `research/private/remc2-level-exporter/` — bounded Level 1 scene exporter.
- `research/private/remc2-level-asset-closure/` — owner-data dependency
  closure builder.
- `research/private/remc2-unity-asset-cache/` — private decoded presentation
  cache builder.
- `research/private/remc2-frame-trace/` — deterministic reference tracing.
- `research/private/remc2-atlas/` — source mapping tools and authored semantic
  documentation; generated and owned data remain local.

The pinned MC2HD/remc2 checkout is represented as a submodule. Local
instrumentation is preserved as a small patch in `upstream/patches/`.

## Data boundary

The project is usable only with lawfully owned data supplied outside Git.
Nothing in this repository grants redistribution rights for the original
game, its assets, or reconstructed expression. See
[`PRIVATE_LAB.md`](PRIVATE_LAB.md) and the provenance policy for the controlling
rules.

