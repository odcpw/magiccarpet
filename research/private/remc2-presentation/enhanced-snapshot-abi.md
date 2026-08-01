# Unity Enhanced snapshot contract

GFX-Q now has an additive, generation-bound presentation contract in the
private Rust path:

- `sim_core::enhanced_presentation::EnhancedPresentationLedger` owns the
  transactional frame boundary, stable source ordering, identity tombstones,
  event backlog, and deterministic state/event/receipt hashes.
- `sim_core::enhanced_presentation::EnhancedFrameState` carries the
  generation/tick-bound profile identity, terrain revision/hash and dirty
  regions, camera and player pose, HUD state, minimap marks, and audio
  references.
- `sim-capi::enhanced_abi` exposes fixed C records with size-query checks and
  bounded arrays. Entity and event pages are independently cursorable; frame
  sidecars are copied through their own bounded buffers.

The ABI is additive and namespaced. Unity receives immutable records; it does
not submit collisions, lights, particles, camera decisions, or gameplay
commands back through this surface. Unknown profile/family codes, generation
zero, stale ticks, duplicate identities, tombstone reuse, invalid cursors,
reserved bytes, null buffers, and undersized buffers fail before output
mutation.

`mc_unity_enhanced_snapshot_page_fixture` and
`mc_unity_enhanced_frame_state_fixture` are deterministic bridge fixtures.
They are marked with `MC_ENHANCED_SNAPSHOT_FLAG_FIXTURE` and exist to lock the
managed/native record layout and boundary behavior while the live
`NativeSession` wiring is added. They are not a claim that Unity already owns
the gameplay session or that a synthetic fixture is a renderer.

Focused gates:

```text
cargo test -p sim-core enhanced_presentation --lib -j1   # 7 passed
cargo test -p sim-capi enhanced_abi --lib -j1            # 4 passed
```

The next integration bead binds a live Rust session to the same ledger and
keeps the Classic HD and Unity Enhanced consumers on the same authoritative
state/checkpoint path.
