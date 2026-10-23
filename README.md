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

Owner game data, generated caches and credentials must remain outside Git and
distributable builds.
