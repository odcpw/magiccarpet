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
They are marked with `MC_ENHANCED_SNAPSHOT_FLAG_FIXTURE` and continue to lock
the managed/native record layout and boundary behavior. The isolated Unity
presentation lab now also has an owner-bound adapter over the existing
native-v2 session, camera, terrain-plane, and entity-copy surfaces. That
adapter produces the same immutable generation-bound Unity snapshot shape and
feeds the URP presentation hierarchy; it does not submit gameplay state back
to Rust. The fixture ABI and the owner-bound v2 route are complementary
surfaces, not a claim that Unity owns simulation authority.

The owner-bound route publishes the Rust reset boundary as an explicit
generation-bound tick-0 snapshot before the first tick-1 step. Render-rate
input is sequence-bound and held across fixed ticks, while cast and selection
edges are consumed once. Terrain planes are copied only when their mutation
sequence changes or the camera crosses a wrapping terrain epoch; the Unity
mesh is rebased as one complete region in the latter case. Nested generation,
tick, identity, finite-pose, array-extent, and quality checks run before the
presentation hierarchy is mutated.

The owner-bound adapter also consumes the existing Rust `sim-capi` v2 event
journal through `mc_native_events_copy_since`. Level 1 projectile and impact
rows are copied at the same generation/tick cursor and are used only to place
presentation VFX; the event journal remains Rust-owned and gameplay events are
never synthesized by Unity. The owner sequence qualification currently proves
one projectile row, one impact row, up to two ordered VFX rows per step, and
generation-bound projectile/impact presentation identities with pooled actor
views and local lights, plus reset-replay equality for those observations. The
same owner qualification pins the action-51 house/parent vector (11, 152, 17)
and proves that each of the 20 production terrain mutations advances the
native mutation sequence and causes exactly one Unity rebuild before an
unchanged settled step produces no terrain publication.

Focused gates:

```text
cargo test -p sim-core enhanced_presentation --lib -j1   # 7 passed
cargo test -p sim-capi enhanced_abi --lib -j1            # 4 passed
```

The remaining integration work is to widen the owner-bound mapping from the
currently covered Level 1 actor/terrain/night presentation slice to the full
coverage matrix and scenario corpus, while keeping Classic HD, Unity
Enhanced, and Exact on the same authoritative Rust checkpoint path.
