# MagicCarpet

A private Rust port of the full Magic Carpet 2 game, using remc2 as the source
reference and Unity as the player/editor. Enemy classes, gameplay properties,
rules and asset bindings are being built for modification and replacement so
the same engine can support other flying-carpet games.

## Source

- [Rust workspace](research/private/remc2-sim-rs/Cargo.toml)
- [Gameplay](research/private/remc2-sim-rs/crates/sim-core/src/lib.rs)
- [Unity client](research/private/remc2-unity-native/Assets)
- [remc2 reference](upstream/magic-carpet-2-hd)
- [Repository instructions](AGENTS.md)

## Build

```sh
cd research/private/remc2-sim-rs
cargo build -p sim-capi --locked
cargo check --workspace --all-targets --locked
cargo fmt --all -- --check
```

Open `research/private/remc2-unity-native` in Unity `6000.3.20f1` for the
desktop client. Existing local builds can be launched from the repository root:

```sh
research/private/remc2-unity-native/run-campaign.sh
```

The player requires matching native code and local owner-data packages/caches.
The full port is in progress. Current support is determined by executable code
and playtesting.

## Development checks

When several exact Unity EditMode checks need the same project, native build
and environment, pass their full method names to one editor launch. For example,
set `project` to the qualified Unity project and `plugin` to its matching native
build artifact, then run from the repository root:

```sh
xvfb-run -a python3 research/private/remc2-unity-native/qualification/run_engine_session_editmode.py \
  --project "$project" --plugin "$plugin" --graphics \
  --results /path/to/new-results.xml --log /path/to/new-editor.log \
  --test-name MagicCarpet.Native.Tests.CampaignMinimapPixelTests.DaySpriteProjectionUsesSourcePitchOffsetAndPixelSpaceRoll \
  --test-name MagicCarpet.Native.Tests.CampaignMinimapPixelTests.ObjectiveMarkDrawsSourceOutlineAndHonorsNativePulseAndMargin
```

The runner checks the native artifact and fails if a requested test is absent,
skipped, duplicated or failed. `--test-filter` selects a whole class. When the
affected tests are uncertain, run the full applicable suite with its normal
owner data and qualification checks.

Run the full Python qualification suite from the repository root:

```sh
python3 -m pytest research/private/remc2-unity-native/qualification/tests
```

The correspondence checks list the Rust tests they name in a single Cargo
invocation and reuse that inventory for the duration of the suite. Cargo
defaults to the repository's disk-backed `target/`. Set `CARGO_TARGET_DIR`
for a gate that requires isolation, or `RCH_REQUIRE_REMOTE=1` to run the
inventory on configured Rig. See [AGENTS.md](AGENTS.md) for Rig and owner-data
requirements.

Owner game data, generated caches and credentials must remain outside Git and
distributable builds.
