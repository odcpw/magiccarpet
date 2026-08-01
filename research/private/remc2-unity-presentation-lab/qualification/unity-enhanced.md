# Unity Enhanced lifecycle shell

This is the private, owner-free GFX-R presentation seam used by the VR graph.
It is not the Exact renderer and it does not run gameplay.

## Contract

- The launcher selects `unity-enhanced` before `UnityEnhancedWorldRoot.Open`.
- `UnityEnhancedWorldRoot` accepts immutable generation/tick-bound snapshots.
- A stale tick, duplicate stable identity, zero identity, or entity from the
  wrong generation is rejected before the current presentation world mutates.
- A reset or generation change recycles the prior presentation objects and
  starts a clean generation. Reused stable identities therefore cannot retain
  transforms from the previous generation.
- The camera rig interpolates presentation poses only. It never writes a camera
  result back to Rust.
- `UnityEnhancedInputAdapter` publishes monotonic sequence-numbered frames and
  consumes each frame once. The future Rust bridge translates those frames into
  the existing command ABI; this shell has no simulation dependency.
- HUD/audio adapters are ordered passive sinks. They do not own collision,
  damage, spell, or other gameplay state.

## Qualification boundary

The static verifier and Unity EditMode tests prove the shell and its failure
rules with synthetic data. They do not claim that the additive Rust snapshot
ABI or a physical XR runtime is complete. Those are downstream GFX-Q and
VR-B/VR-C acceptance gates. Owner package, closure, and asset-cache inputs stay
external to the lab.

## Commands

```text
python3 qualification/verify_unity_enhanced.py --json
python3 -m unittest discover -s qualification/tests -p 'test_*.py' -v
Unity -batchmode -nographics -projectPath . -runTests -testPlatform EditMode \
  -assemblyNames Assembly-CSharp-Editor -testResults /tmp/unity-enhanced.xml \
  -logFile /tmp/unity-enhanced.log
```
