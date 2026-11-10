# Level1 settlement human conversion recipe

This is the four-look villager and Builder/Trader human/garment pipeline, not an automatic
image-to-rig system. It reuses the accepted editable MPFB body, its own Rigify
actions and the free separate robe. The lower robe uses the reviewed offline
cloth-to-48-bone bake described in [CLOTH_SKINNING.md](CLOTH_SKINNING.md).
No runtime cloth solver or paid 3D generation was used.

Builder and Trader reuse the same human/rig and five actions, with separately
authored short vests/trousers and head/pack props. They do not reuse a robe
silhouette. Day/Night references for parameters221/219 are in generated/
settlement-people-20261110/reference-{day,night}-complete. The original Level1
cache omitted their three TMAP blocks; those references decode the real owner
TMAPS0/1 DAT/TAB records138–143 with the pinned RNC decoder. Source-bank hashes
are retained in source-bank-cache/provenance.json. No inferred tool or invented
hammer/work clip is added to the Builder.

## Source and art bindings

| Parameter | Reset count | TMAP views | Logical height | Resource / generator |
| --- | ---: | --- | ---: | --- |
| 239 | 3 | 155–157 | 200 | VillagerBrown / villager_brown.py |
| 241 | 6 | 180–182 | 250 | Villager / accepted pilot |
| 242 | 9 | 183–185 | 250 | VillagerStriped / villager_striped.py |
| 271 | 10 | 199–201 | 300 | VillagerCarrier / villager_carrier.py |
| 221 | Conditional house production | 141–143 | 250 | Builder/model12 / builder_character.py |
| 219 | Conditional house production | 138–140 | 250 | Trader/model14 / trader_character.py |

The four reset appearances are native class5/model13. All six are class5. Source heights come from
sim-definition/adapter.rs appearance constants; Unity divides by64. The carrier
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

## Previous villager-roster evidence and limits

That installation's run-modern.sh/run-voxels.sh selected generated/villager-roster-20261110/
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

## Builder and Trader extension

Use fresh OUT directories, one per asset. The same pinned M and B above apply.

```sh
"$B" -b --python-exit-code 2 --python "$P/builder_character.py" -- --prepare --package --output "$OUT"
# Trader uses its own fresh OUT, then reopens that editable master.
"$B" -b "$M" --python-exit-code 2 --python "$P/trader_character.py" -- --author "$OUT"
"$B" -b "$OUT/master/Trader.blend" --python-exit-code 2 --python "$P/trader_character.py" -- --audit "$OUT"
"$B" -b "$OUT/master/Trader.blend" --python-exit-code 2 --python "$P/trader_character.py" -- --gait "$OUT"
"$B" -b "$OUT/master/Trader.blend" --python-exit-code 2 --python "$P/trader_character.py" -- --package "$OUT"
```

Final owner-local packages are builder/candidate07/export and
trader/candidate09/export under generated/settlement-people-20261110.
They have172/173 deform bones. Both pass25 actual Unity surface poses within
0.1mm, including the actual pack; whole-costume quarter-frame minima are
−0.820/−0.833mm. Trader gait is measured again at1.429990/3.728282m/s.
The grip is approximate. Full editable anatomy stays in the master; hidden
under-garment triangle intersections are diagnostics, not an automatic failure.
The attempted masked Trader opened a visible waist hole and was rejected.
Check visible boundaries and all five moving silhouettes. Every action must
reset released-prop transforms; the first pack stayed detached after Death.

Native admission must reach both sim-capi character_abi.rs and Unity's
NativeCampaignSession.CharacterStates reader, then the shared appearance
registry, actor presenter, importer and voxel importer. An initial real run
caught the reader's old model13-only whitelist even though the standalone
art checks passed. Preserve strict class/model/appearance, layout, tick and
lifetime validation; adding an art resource alone is insufficient.

Builder hit102 (or110 after the source post-build handoff) and Trader hit118
use the shared moving overlay with their own actions. Builder100/101 and
Trader117 are deaths; Builder108/109 with positive life and Trader116 house
entry must not invent a fall or fragments. Hide flags apply in both styles.
A confirmed death remains visible through the hidden native reclaim tail,
avoiding a one-frame disappearance before its retained Modern corpse.

Public Level1 inputs genuinely demolish house16 and create Builder454/gen28;
one branch kills it (life−1, one retained death), another builds house16/gen1/
BUILD39 and enters house38 at life999 with no death. Both120-tick cuts match
fresh native instances. The authentic direct Level7 package exposes four
Traders and positive-life entry/reclaim; its1200-input fresh-instance comparison
passes. This is not Level1 Trader production or a Trader combat/death proof.
That direct neutral session cannot use GameRuntime save or voxel-enable APIs.
One final public route genuinely acquires Whirlwind at tick23; its2000mana
cost exceeds the direct reset's1000maximum, so casts do not damage the Trader.
Continuing that route reproduces native status8 at tick1139 (target slot53
missing its current shared spell record) in two fresh instances. This is a
shared engine defect, not a reason to grant mana or call the asset complete.
These limits remain open in mc-oi5; never modify a save or grant a spell to
manufacture the missing evidence. Detailed source/command/hash evidence is
generated/settlement-people-20261110/native/PROOF.md.

Current run-modern.sh/run-voxels.sh select generated/settlement-people-20261110/
player-reader, presentation source7f554577, nativead803e03 from the isolated
65724d7b source patch. The previous installation remains at
Builds/Presentation-before-settlement-people-20261110. Both actual default
launchers recorded180frames of the real Builder kill and once-only Voxel
breakup; public F5/R plus fresh loads preserve a live Builder then restore28
villagers on reset. Checked16.5s portrait media is
builder-trader-installed-checked.mp4. Builder mc-jcy is complete; Trader
mc-oi5 retains its full unverified runtime acceptance.

Private3090/KWin/Vulkan1280×960 no-readback medians are218/113FPS Modern/Voxel
(two runs/style), versus229/120 for a contemporaneous old-player comparison.
The current measurements are slower; no speedup is claimed. Native throughput
is~27.99/27.24ticks/s and>100ms frame outliers remain. The test does not establish
causation or whole-level/desktop/VR performance. Full distributions and source
hashes are in integration/performance-comparison.json and presentation-install.json.
