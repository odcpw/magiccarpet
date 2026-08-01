# Private MC2 VR architecture gate

Status: handoff recorded; downstream runtime work is still gated.

## Decision

The VR mode is realistic if it is treated as a fourth, mutually exclusive
presentation mode over the accepted Unity Enhanced path. It is not realistic
as a stereo wrapper around the sealed Exact `FINAL MAIN` renderer. Rust keeps
ownership of flight, spells, projectiles, collisions, objectives, RNG, saves,
replay hashes and all other gameplay truth. Unity receives snapshots/events and
emits the same bounded commands that a flat client emits.

The first input slice is deliberately conservative:

1. The left controller is a calibrated yoke: pitch/roll for carpet attitude
   and thumbstick axes for forward/reverse/strafe.
2. The right controller aims through its pose, normalized into the existing
   camera/aim ABI; its trigger invokes the existing aimed cast.
3. Recenter, handedness, tracking-loss behavior and non-gesture fallbacks are
   mandatory.
4. Hand-origin physical throwing is a separate experiment after the aimed cast
   slice and is not part of the first acceptance gate.

## Handoff and ownership

GFX-A is pinned at `35aa572d79e971dd4623993172246a5c035e1e9e`, GFX-B at
`1bd6af35076e6e209a5a6b1bb82959cee6b4493d`, and the isolated lab scaffold is
handed off at `696ef02d026056f8499f7cf8da6535e7d4d33ffd`. The lab lives at
`research/private/remc2-unity-presentation-lab`; it has separate packages,
settings, scenes and cache/build roots. The handoff currently has static
guard evidence, while the Unity editor/player smoke remains pending because
neither `t14` nor `legion` currently has the pinned Unity editor installed.

The Exact project is frozen at
`research/private/remc2-unity-native`. VR work cannot edit it, its qualification
corpus, or its caches. The Level 2 exporter/package surfaces are explicitly
outside the VR reservation. The rollback point before the lab commit is
`main@bf6614dadd13fb37aa069a0760acbd2e98660f39`; unrelated dirty work must be
preserved during any later integration.

The explicit downstream dependencies are the Exact guard (`mc-dl0r.18.1`),
three-mode contract (`mc-dl0r.18.2`), presentation lab (`mc-dl0r.18.4`), and
the additive Rust presentation ABI (`mc-dl0r.18.13`). The first two currently
exist as isolated branch baselines rather than shared-checkout integrations;
that is recorded as a handoff condition instead of being silently treated as
merged.

## Runtime evidence

The first real runtime candidate is `legion` over the Tailscale network:

- Windows build `10.0.26200.8973`.
- NVIDIA GeForce RTX 3090, driver `32.0.15.9186`.
- SteamVR is installed at `C:\Program Files (x86)\Steam\steamapps\common\SteamVR`.
- The Windows OpenXR registry selects
  `C:\Program Files (x86)\Steam\steamapps\common\SteamVR\steamxr_win64.json`.
- No physical Quest/Vive/Index headset or controller was visible in the
  present-device query on 2026-08-01.
- `t14` has Steam and Vulkan, but no Unity editor and no connected headset.

The `tailscale ssh legion` wrapper currently fails before authentication
because this client has no known ED25519 host key and the advertised Tailscale
SSH host-key field is empty. The read-only probe used the equivalent
Tailscale-proxied OpenSSH route:

```text
ssh -o 'ProxyCommand=tailscale nc %h %p' legion <read-only-command>
```

That is a transport workaround, not a support claim. Before any remote build
or install, the host-key/Tailscale SSH setup must be repaired or explicitly
approved.

## Comfort and performance gate

The default is seated-first, with a stable carpet reference, no forced visual
roll, 30-degree snap turn, comfort-ramped acceleration, adjustable vignette,
reduced camera shake/flash, and an explicit recenter action. The target frame
budgets are 13.89 ms at 72 Hz and 11.11 ms at 90 Hz; acceptance requires p99
frame time within the selected budget through representative flight and
combat, zero steady-state allocations after warmup, Rust tick p99 at or below
4 ms, and no sustained reprojection/dropped-frame burst. The private PC gate
also records p95 working set (4 GiB), startup (20 s), suspend/resume (5 s),
tracking-loss recovery (2 s), and a 30-minute thermal soak. Quest 3 uses a
separate Android ARM64 measurement gate; Quest 2 is optional and can be
rejected without invalidating the PC/Quest 3 path.

## Stop rules

Accept only after the editor resolves the lab lock, the empty OpenXR build
starts/exits, both modes boot synthetic/no-owner, the PC route passes the
numeric/comfort gates, replay/state hashes match the flat Enhanced route, and
the Exact negative scan remains clean. Limit the profile when runtime support
exists without a connected headset, when Quest data is unmeasured, or when a
visual mismatch has not been shown to affect Rust truth. Reject any design
where Unity writes gameplay state, XR packages enter Exact, hand pose bypasses
command validation, owner payload is bundled, or comfort/performance requires a
rules fork.
