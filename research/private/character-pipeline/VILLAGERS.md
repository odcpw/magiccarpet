# Level1 villager conversion recipe

This is the installed four-look human/garment pipeline, not an automatic
image-to-rig system. It reuses the accepted editable MPFB body, its own Rigify
actions and the free separate robe. The lower robe uses the reviewed offline
cloth-to-48-bone bake described in [CLOTH_SKINNING.md](CLOTH_SKINNING.md).
No runtime cloth solver or paid 3D generation was used.

## Source and art bindings

| Parameter | Reset count | TMAP views | Logical height | Resource / generator |
| --- | ---: | --- | ---: | --- |
| 239 | 3 | 155–157 | 200 | VillagerBrown / villager_brown.py |
| 241 | 6 | 180–182 | 250 | Villager / accepted pilot |
| 242 | 9 | 183–185 | 250 | VillagerStriped / villager_striped.py |
| 271 | 10 | 199–201 | 300 | VillagerCarrier / villager_carrier.py |

All are native class5/model13. Source heights come from
sim-definition/adapter.rs::VILLAGER_APPEARANCES; Unity divides by64. The carrier
height includes the jar. Day and Night source sheets are under generated/
villager-roster-20261110/reference-day and generated/villager-modern-20261109/
reference respectively. Night blue/dark colours must not be mistaken for
neutral fabric:239 is brown,242 is pale grey/lavender striped,271 carries a
tan vessel. Do not infer an unviewed appearance from a label.

## Rebuild owner-local packages

Run from the repository root. Set OUT to a fresh path each time. Body and
costume remain separate in the editable master; the portable export joins
disconnected components only. The input master is generated/
villager-modern-20261109/runtime-character/master/upper-prepared.blend,
SHA256 c36f1f0f8fcec31d9e0364faa5f8eacd4d45657d95aaf4796caf8dc52ab765c0.

```sh
B=/home/oliver/Projects/fittimer-avatar-toolchain/blender-4.5.12-linux-x64/blender
P=research/private/character-pipeline
M=research/private/generated/villager-modern-20261109/runtime-character/master/upper-prepared.blend
# Brown: complete authored master, audit, materials and portable package.
"$B" -b --python-exit-code 2 --python "$P/villager_brown.py" -- --master "$M" --package --output "$OUT"
# Striped: its own fresh OUT; includes real woven albedo and authored UVs.
"$B" -b --python-exit-code 2 --python "$P/villager_striped.py" -- --prepare --package --output "$OUT"
# Carrier: its own fresh OUT, then use that same path for audit/package.
"$B" -b "$M" --python-exit-code 2 --python "$P/villager_carrier.py" -- --author "$OUT"
"$B" -b "$OUT/master/VillagerCarrier.blend" --python-exit-code 2 --python "$P/villager_carrier.py" -- --audit "$OUT"
"$B" -b "$OUT/master/VillagerCarrier.blend" --python-exit-code 2 --python "$P/villager_carrier.py" -- --package "$OUT"
```

Accepted exports are brown/export, striped/candidate03/export and
carrier/candidate08/runtime under generated/villager-roster-20261110. Carrier
FBX SHA256 is e6ba6204531109b42d1150cd4ef4756624f692f980375da9312853c2de514f33.
The installed carrier manifest additionally declares uprightSupportBone and
uprightFootRadius; the script's --derive-manifest/--output mode adds these to a
fresh manifest without regenerating or modifying the original FBX/package.
The integrated manifest is integration/carrier-supported-manifest.json.

All have Idle/Walk/Run/Hit/Death at120Hz, measured authoring Walk/Run speeds
1.4300195/3.7283345m/s and explicit moving-hit bones. Runtime gait follows
rendered native travel at the correct appearance scale. There are224 deform
bones,225 for the carrier. FBX/material/manifest outputs go through
ModernCharacterImport and same-master VoxelGoatImport at64/96 resolution.
Preserve GUIDs when copying prepared owner-local resources to the canonical
Assets/Resources/ModernCharacters folder. Generated assets stay ignored.

## Gates and failure modes

Use export_character_fbx.py's source/FBX surface checks and actual Unity
PortableSkinProbe:25 poses across all five actions, fixed axis conversion,
bidirectional0.1mm gate. The earlier rendered-point negative control rejects.
Full costume checks retain the existing3mm ground gate; inherited Walk minimum
is−0.820mm. This does not prove exact collision-free cloth. Inspect full front/
side/moving/settled frames; success metrics cannot approve a broken silhouette.

The held vessel declares uprightBone, uprightSupportBone and its measured
0.08m foot radius. Runtime uses cached rigid crown vertices, aligns the jar to
world gravity and supports its foot after terrain tilt and progressed recoil.
No per-frame BakeMesh. Reapply before a last-live-pose Voxel snapshot; leave
released Death alone. Rotation-only correction caused up to38.6mm crown
penetration in authoring scale and was rejected. The regression independently
bakes actual posed geometry on both slope signs, progresses one hit rather
than restarting it every sample, and checks the changed-tilt fracture path.

Repeated Fireballs previously exhausted the768-body debris pool and hid deaths
without a breakup. Keep the budget and64 activations/frame. Bound impact
reservations, prioritize admitted character bursts and recycle eligible older
pieces. Test repeated impacts/deaths, pending accounting and reset, then verify
one actual burst per native lifetime. Never fake a death or increase the pool
to hide starvation. Continue native observation while culling unseen rigs;
resuming travel must rebase interpolation and invisible deaths must finish.

## Installed evidence and limits

Normal run-modern.sh/run-voxels.sh select generated/villager-roster-20261110/
player-view (source9cf9159f); native remains qualified e8a6e707 from eaf0f1bc.
Previous Presentation is preserved at Builds/Presentation-before-villager-roster-20261110.
Actual default launchers independently recorded the carrier kill without a
player-path override. R/F5 and cold loads retain28 unique residents with3/6/9/10
look counts. Public-input kills:239/slot53/tick352,242/slot54/tick415,
271/slot51/tick350. Cold native replay reproduces all states and reclamation.
Modern retains the authored corpse; Voxel emits one breakup per lifetime.

Under generated/villager-roster-20261110/integration: final-live-proof.json,
default-launchers-proof.json, install-switch.json, cold-route-proof.json,
lifecycle-view-*/{result,cold-proof}.json, surface proofs, integrated-tests.xml
(105pass/one existing optional profiling skip), view-tests.xml(53pass), and
view-performance-comparison.json. UBS C# analysis is partial owing to callable/
taint work limits; findings were reviewed, not presented as a clean scan.
The13.4s portrait villagers-installed-checked.mp4 distinguishes bright Unity
model review from live Level1 play; its ending uses both actual normal launchers.
Private KWin/3090 Vulkan1280×960, no readback, two runs/style measured
~222/120FPS Modern/Voxel and~28.05/27.17native ticks/s. Modern was~160FPS before
the view gate, with identical assets. >100ms frame spikes remain. This is not
an unlocked-desktop, VR, complete-Level1 or full-campaign performance claim.

This recipe directly serves the next humanoid variants. A different body,
garment cut, tool interaction or creature skeleton needs its own fit/actions
and the same real consumer checks; do not assume these five clips suffice.
