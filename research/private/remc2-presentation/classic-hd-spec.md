# Classic HD source contract

`classic-hd-spec.json` is the implementation contract for the real HD path.
It is pinned to remc2 commit
`6806c7204d17220ecc0430759f784aea8ee2ddb2`; source facts must be re-read
from that git object, never from the dirty upstream checkout.

The key distinction is architectural: Classic HD rerenders the indexed world
into a larger profile-sized buffer. It does not take the Exact 320x200 FINAL
MAIN and enlarge the finished image. Window size, internal game resolution,
source-style HUD scale, optional high-resolution owner banks, and presentation
history are separate concepts.

Pinned remc2’s regression profile supplies the first implementation target:
640x480 game and window resolution, maintained aspect ratio, source assets,
sky, reflections, and dynamic lighting. The roadmap adds a 1280x960 4:3
profile for testing buffer generalization. Widescreen world expansion is
deferred; widescreen windows are fitted with pillarboxing and the source HUD
remains integer-scaled.

The source census records the important seams:

- `GameRenderHD::GameRenderHD` owns resolution-bounded indexed buffers and
  three-plane blur padding; `GameRenderHD::~GameRenderHD` defines lifetime.
- `DrawWorld_411A0` and `DrawTerrainAndParticles_3C080` consume the active
  viewport and `viewDistanceScale`; tile lattice and fog cutoffs scale with
  the selected render profile.
- `DrawSky_40950_TH`/`DrawSky_40950` render an indexed sky into the viewport,
  selecting the base 256 bank or the optional 1024 high-resolution bank.
- `SubDraw*TerrainAndParticles`, `DrawTriangleInProjectionSpace_B6253`, and
  `CheckViewPortCull` define the terrain/painter/clipping stage.
- `SetBillboards_3B560`, `DrawSprites_3E360`, and `DrawSprite_41BD3` define
  typed billboard and sprite projection.
- `Basic.cpp` owns `gameUiScale`, source 640-wide HUD placement, active screen
  dimensions, and pointer mapping.
- `EventsFunctions.cpp` selects `GameRenderHD` and screen dimensions before
  the render object is constructed.

The JSON pipeline table gives each stage its inputs, output, order, and a
discriminating synthetic test. Unresolved reflection/blur ownership,
temporal-history equivalence, optional sprite-bank coverage, and unsupported
branches fail closed until their tests exist.

Validate the contract with:

```bash
python3 research/private/remc2-presentation/validate_classic_hd_spec.py
python3 -m unittest discover \
  -s research/private/remc2-presentation/tests -p 'test_*.py' -v
```
